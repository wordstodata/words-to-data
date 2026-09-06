//! In-memory storage backend

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, HouseRollCall, Member, SponsorInfo, VotePosition};
use crate::dataset::{
    DatasetError, DatasetMetadata, Expression, ExpressionId, ExpressionInfo, ExpressionPair,
    SearchResult, WorkId,
};
use crate::diff::TreeDiff;
use crate::intern::StringInterner;
use crate::storage::{
    DocumentReader, DocumentWriter, LegislatureReader, LegislatureWriter, LinkReader, LinkWriter,
    Storage,
};
use crate::uslm::USLMElement;
use crate::uslm::bill_parser::Bill;

/// Custom serializer for HashMap with tuple keys (JSON doesn't support non-string keys)
mod tuple_key_map {
    use super::*;

    pub fn serialize<S>(
        map: &HashMap<ExpressionPair, Vec<ChangeAnnotation>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<_> = map.iter().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<ExpressionPair, Vec<ChangeAnnotation>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<(ExpressionPair, Vec<ChangeAnnotation>)> = Vec::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

/// Every expression a dataset holds, by work and then by date.
///
/// Two nested `BTreeMap`s rather than one flat list: work order and date order
/// then come for free, and "the expression before this one" can only ever mean
/// the previous expression *of the same work*. A flat list keyed by date made
/// that question answerable across unrelated documents, which is the bug this
/// shape removes.
pub type ExpressionsByWork = BTreeMap<WorkId, BTreeMap<String, Expression>>;

/// In-memory storage backend
///
/// Stores all data in memory using maps. Good for small datasets
/// or when you need fast access without persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryStorage {
    pub metadata: DatasetMetadata,
    pub expressions: ExpressionsByWork,
    pub bills: HashMap<String, Bill>,
    #[serde(with = "tuple_key_map")]
    pub diff_annotations: HashMap<ExpressionPair, Vec<ChangeAnnotation>>,
    pub members: HashMap<String, Member>,
    pub sponsors: HashMap<String, SponsorInfo>,
    pub bill_votes: HashMap<String, BillVotes>,
    #[serde(skip)]
    interner: StringInterner,
}

impl InMemoryStorage {
    pub fn new(metadata: DatasetMetadata) -> Self {
        Self {
            metadata,
            expressions: BTreeMap::new(),
            bills: HashMap::new(),
            diff_annotations: HashMap::new(),
            members: HashMap::new(),
            sponsors: HashMap::new(),
            bill_votes: HashMap::new(),
            interner: StringInterner::new(),
        }
    }

    pub fn intern_strings(&mut self) {
        for expression in self.expressions.values_mut().flat_map(BTreeMap::values_mut) {
            expression.element.intern_strings(&mut self.interner);
        }
    }

    /// Every expression held, oldest first within each work.
    pub fn all_expressions(&self) -> impl Iterator<Item = &Expression> {
        self.expressions.values().flat_map(BTreeMap::values)
    }

    /// Create from parts (used by loaders)
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        metadata: DatasetMetadata,
        expressions: ExpressionsByWork,
        bills: HashMap<String, Bill>,
        diff_annotations: HashMap<ExpressionPair, Vec<ChangeAnnotation>>,
        members: HashMap<String, Member>,
        sponsors: HashMap<String, SponsorInfo>,
        bill_votes: HashMap<String, BillVotes>,
        interner: StringInterner,
    ) -> Self {
        Self {
            metadata,
            expressions,
            bills,
            diff_annotations,
            members,
            sponsors,
            bill_votes,
            interner,
        }
    }

    /// The dates of one work, oldest first, or an empty slice when unheld.
    fn dates_of(&self, work: &WorkId) -> Vec<&String> {
        self.expressions
            .get(work)
            .map(|by_date| by_date.keys().collect())
            .unwrap_or_default()
    }

    /// The expression `offset` places from `id` within its own work.
    fn neighbour(&self, id: &ExpressionId, offset: isize) -> Option<&Expression> {
        let by_date = self.expressions.get(&id.work)?;
        let position = by_date.keys().position(|date| *date == id.at)?;
        let wanted = position.checked_add_signed(offset)?;
        by_date.values().nth(wanted)
    }

    fn search_element(
        element: &USLMElement,
        id: &ExpressionId,
        query: &str,
        results: &mut Vec<SearchResult>,
    ) {
        let fields = [
            ("heading", &element.data.heading),
            ("chapeau", &element.data.chapeau),
            ("content", &element.data.content),
            ("proviso", &element.data.proviso),
            ("continuation", &element.data.continuation),
        ];

        for (field_name, field_value) in fields {
            if let Some(text) = field_value
                && text.to_lowercase().contains(query)
            {
                results.push(SearchResult {
                    expression: id.clone(),
                    path: element.data.path.to_string(),
                    field: field_name.to_string(),
                    snippet: text.to_string(),
                });
            }
        }

        for child in &element.children {
            Self::search_element(child, id, query, results);
        }
    }
}

impl DocumentReader for InMemoryStorage {
    fn works(&self) -> Result<Vec<WorkId>, DatasetError> {
        Ok(self.expressions.keys().cloned().collect())
    }

    fn expressions(&self, work: &WorkId) -> Result<Vec<ExpressionInfo>, DatasetError> {
        Ok(self
            .dates_of(work)
            .into_iter()
            .map(|date| ExpressionInfo {
                id: ExpressionId::new(work.clone(), date),
                label: self.expressions[work][date].label.clone(),
            })
            .collect())
    }

    fn get_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        Ok(self
            .expressions
            .get(&id.work)
            .and_then(|by_date| by_date.get(&id.at))
            .cloned())
    }

    fn next_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        Ok(self.neighbour(id, 1).cloned())
    }

    fn prev_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        Ok(self.neighbour(id, -1).cloned())
    }

    fn compute_diff(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError> {
        let (from_e, to_e) = require_same_work(self, from, to)?;
        Ok(TreeDiff::from_elements(&from_e.element, &to_e.element))
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for expression in self.all_expressions() {
            Self::search_element(
                &expression.element,
                &expression.id,
                &query_lower,
                &mut results,
            );
        }

        Ok(results)
    }

    fn find_element(&self, path: &str) -> Result<Vec<(ExpressionId, USLMElement)>, DatasetError> {
        // A path can name more than one provision, so an expression can answer
        // with several. Taking the first would drop law that is really there.
        Ok(self
            .all_expressions()
            .flat_map(|e| {
                e.element
                    .find_all(path)
                    .into_iter()
                    .map(|found| (e.id.clone(), found.clone()))
                    .collect::<Vec<_>>()
            })
            .collect())
    }
}

/// Fetch both expressions of a diff, refusing a pair that names two works.
///
/// The check lives here rather than in `TreeDiff::from_elements`, which asserts
/// on it and so would abort the process. Two works is a caller's mistake, not a
/// broken invariant, and a mistake deserves a message.
pub(crate) fn require_same_work<R: DocumentReader + ?Sized>(
    reader: &R,
    from: &ExpressionId,
    to: &ExpressionId,
) -> Result<(Expression, Expression), DatasetError> {
    if from.work != to.work {
        return Err(DatasetError::WorkMismatch {
            from: from.work.clone(),
            to: to.work.clone(),
        });
    }
    let from_e = reader
        .get_expression(from)?
        .ok_or_else(|| DatasetError::ExpressionNotFound(from.clone()))?;
    let to_e = reader
        .get_expression(to)?
        .ok_or_else(|| DatasetError::ExpressionNotFound(to.clone()))?;
    Ok((from_e, to_e))
}

impl LinkReader for InMemoryStorage {
    fn get_annotations(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        let key = (from.clone(), to.clone());
        Ok(self.diff_annotations.get(&key).cloned())
    }

    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        Ok(self
            .diff_annotations
            .values()
            .flatten()
            .filter(|a| a.paths.iter().any(|p| p == path))
            .cloned()
            .collect())
    }

    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        Ok(self
            .diff_annotations
            .values()
            .flatten()
            .filter(|a| a.source_bill.bill_id == bill_id)
            .cloned()
            .collect())
    }

    fn annotation_pairs(&self) -> Result<Vec<ExpressionPair>, DatasetError> {
        let mut pairs: Vec<ExpressionPair> = self.diff_annotations.keys().cloned().collect();
        pairs.sort();
        Ok(pairs)
    }
}

impl LegislatureReader for InMemoryStorage {
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError> {
        Ok(self.bills.get(id).cloned())
    }

    fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError> {
        Ok(self.bills.keys().cloned().collect())
    }

    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        Ok(self.members.get(bioguide_id).cloned())
    }

    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        Ok(self.sponsors.get(bill_id).cloned())
    }

    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        Ok(self.bill_votes.get(bill_id).cloned())
    }

    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        let mut results = Vec::new();
        for bill_votes in self.bill_votes.values() {
            for roll_call in &bill_votes.roll_calls {
                for mv in &roll_call.member_votes {
                    if mv.bioguide_id == bioguide_id {
                        results.push((roll_call.clone(), mv.position));
                    }
                }
            }
        }
        Ok(results)
    }
}

impl DocumentWriter for InMemoryStorage {
    fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }

    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.metadata = metadata;
    }

    fn add_expression(&mut self, expression: Expression) -> Result<(), DatasetError> {
        self.expressions
            .entry(expression.id.work.clone())
            .or_default()
            .insert(expression.id.at.clone(), expression);
        Ok(())
    }
}

impl LinkWriter for InMemoryStorage {
    fn add_annotation(
        &mut self,
        from: &ExpressionId,
        to: &ExpressionId,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError> {
        self.diff_annotations
            .entry((from.clone(), to.clone()))
            .or_default()
            .push(annotation);
        Ok(())
    }
}

impl LegislatureWriter for InMemoryStorage {
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        self.bills.insert(bill.bill_id.clone(), bill);
        Ok(())
    }

    fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        self.members.insert(member.bioguide_id.clone(), member);
        Ok(())
    }

    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        self.sponsors.insert(info.bill_id.clone(), info);
        Ok(())
    }

    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        self.bill_votes.insert(votes.bill_id.clone(), votes);
        Ok(())
    }
}

impl Storage for InMemoryStorage {
    fn legislature(&self) -> Option<&dyn LegislatureReader> {
        let holds_legislature = !self.bills.is_empty()
            || !self.members.is_empty()
            || !self.sponsors.is_empty()
            || !self.bill_votes.is_empty();
        holds_legislature.then_some(self as &dyn LegislatureReader)
    }
}
