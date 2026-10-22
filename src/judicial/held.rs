//! The court opinions a dataset holds, each with the facts a reader needs to
//! name it (#308).
//!
//! `expressions` names a case as `judicial/case_<cluster>@<date>` and nothing
//! more. An agent that saw only that wrote opinion ids from memory, and could
//! not tell which writing of a case said what. This list gives one row for
//! each writing, with its case, and fetches nothing.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::dataset::DatasetError;
use crate::document::{DocumentNode, NodeType};
use crate::link::LinkKind;
use crate::storage::{DocumentReader, LinkReader};

use super::{CaseFacts, OpinionFacts};

/// One writing the dataset holds, as one row of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeldOpinion {
    /// The writing's path, such as `judicial/case_122262/opinion_9434365`.
    /// `path`, `search --path` and a citation's subject take it.
    pub path: String,
    /// The case the writing belongs to: its work, such as
    /// `judicial/case_122262`.
    pub case: String,
    /// The day the court filed the case: the date of its expression.
    pub date: String,
    /// The case as it is cited, such as `Boeing Co. v. United States`. `None`
    /// when the stored case carries no judicial facts and no heading.
    pub case_name: Option<String>,
    /// What kind of writing it is, in the publisher's vocabulary, such as
    /// `020lead`, `040dissent`, or `010combined` for one record that holds the
    /// whole case.
    pub opinion_type: Option<String>,
    /// The judge the publisher names as its author, when it names one.
    pub author: Option<String>,
    /// The publisher's id for the case: the cluster.
    pub cluster_id: Option<u64>,
    /// The field of the publisher's record the text was read from, such as
    /// `plain_text` or `xml_harvard`, from the node's provenance
    /// (`docs/adr/0008-an-opinions-text-is-one-named-field-chosen-for-fidelity.md`).
    /// A source in another form is given whole. `None` when no provenance is
    /// recorded.
    pub text_field: Option<String>,
    /// The publisher's combined records of the case that hold the same words
    /// as its writings and are **not** stored as text
    /// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
    /// decision 2). Empty when every record of the case is stored.
    pub combined_records: Vec<u64>,
    /// The court that filed it. Always `None` today: the stored facts hold the
    /// docket and not the court, because the court is one more request per case
    /// (#53). A reader says "not recorded" and fetches nothing.
    pub court: Option<String>,
    /// How many `judicial.cites` links name this writing, or a path beneath it,
    /// as their subject: the U.S. Code citations the dataset holds for it.
    pub cites: usize,
}

/// Every writing the dataset holds, one row for each, ordered by date, then by
/// cluster, then in the order the case prints its writings.
///
/// A judicial work is found by the first segment of its path, which names the
/// document class (`docs/adr/0006-a-document-node-is-class-neutral.md`). A
/// writing is a `judicial.opinion` node: a child of its case, or, in a dataset
/// built before cases were works, the root of its own work.
pub fn held_opinions<R: DocumentReader + LinkReader + ?Sized>(
    reader: &R,
) -> Result<Vec<HeldOpinion>, DatasetError> {
    let cites = citations_by_subject(reader)?;
    let mut rows = Vec::new();
    for work in reader.works()? {
        if work.as_str().split('/').next() != Some(NodeType::JUDICIAL) {
            continue;
        }
        for info in reader.expressions(&work)? {
            let Some(expression) = reader.get_expression(&info.id)? else {
                continue;
            };
            let root = &expression.root;
            let case = CaseFacts::of(&root.data);
            for (order, writing) in writings_of(root).into_iter().enumerate() {
                let data = &writing.data;
                let facts = OpinionFacts::of(data).unwrap_or_default();
                rows.push((
                    order,
                    HeldOpinion {
                        path: data.path.to_string(),
                        case: work.to_string(),
                        date: info.id.at.clone(),
                        case_name: case
                            .as_ref()
                            .map(|case| case.case_name.clone())
                            .filter(|name| !name.is_empty())
                            .or_else(|| root.data.heading.as_deref().map(str::to_string)),
                        opinion_type: facts.opinion_type,
                        author: facts.author,
                        cluster_id: case.as_ref().map(|case| case.cluster_id),
                        text_field: data
                            .provenance
                            .as_ref()
                            .map(|provenance| text_field_of(&provenance.source)),
                        combined_records: case
                            .as_ref()
                            .map(|case| case.combined_records.clone())
                            .unwrap_or_default(),
                        court: None,
                        cites: cites
                            .iter()
                            .filter(|(subject, _)| {
                                crate::uslm::path::covers_path(&data.path, subject)
                            })
                            .map(|(_, count)| count)
                            .sum(),
                    },
                ));
            }
        }
    }
    // The writings of one case share a cluster and its filing date, so this
    // order keeps them together, in the order the case prints them, and the
    // list reads oldest first.
    rows.sort_by(|(a_order, a), (b_order, b)| {
        (&a.date, a.cluster_id, &a.case, a_order).cmp(&(&b.date, b.cluster_id, &b.case, b_order))
    });
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

/// The writings in a stored judicial tree: the root when it is one, and every
/// child that is one, in document order.
fn writings_of(root: &DocumentNode) -> Vec<&DocumentNode> {
    let is_writing = |node: &DocumentNode| node.data.node_type.as_str() == NodeType::OPINION;
    if is_writing(root) {
        return vec![root];
    }
    root.children
        .iter()
        .filter(|child| is_writing(child))
        .collect()
}

/// How many `judicial.cites` links each path is the subject of, read once.
///
/// The subject of a citation is the writing that made it.
fn citations_by_subject<R: LinkReader + ?Sized>(
    reader: &R,
) -> Result<BTreeMap<String, usize>, DatasetError> {
    let mut counts = BTreeMap::new();
    for link in reader.links_by_kind(LinkKind::CITES)? {
        if let Some(subject) = link.subject.path() {
            *counts.entry(subject.to_string()).or_insert(0) += 1;
        }
    }
    Ok(counts)
}

/// The field named at the end of a CourtListener text source, such as
/// `plain_text` in `courtlistener:opinion/2651100:plain_text`. Any other source
/// is returned whole, so nothing is dropped.
fn text_field_of(source: &str) -> String {
    source
        .strip_prefix("courtlistener:opinion/")
        .and_then(|rest| rest.split_once(':'))
        .map_or(source, |(_, field)| field)
        .to_string()
}
