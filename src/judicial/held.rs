//! The court opinions a dataset holds, each with the facts a reader needs to
//! name it (#308).
//!
//! `expressions` names an opinion as `judicial/opinion_<id>@<date>` and nothing
//! more. An agent that saw only that wrote opinion ids from memory, and could
//! not tell that two works were the majority and the dissent of one case. This
//! list reads the facts the dataset already stores, and fetches nothing.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::dataset::DatasetError;
use crate::document::NodeType;
use crate::link::LinkKind;
use crate::storage::{DocumentReader, LinkReader};

use super::OpinionFacts;

/// One opinion the dataset holds, as one row of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeldOpinion {
    /// The opinion's work, such as `judicial/opinion_2812209`.
    pub work: String,
    /// The day the court filed it: the date of its expression.
    pub date: String,
    /// The case as it is cited, such as `Obergefell v. Hodges`. `None` when
    /// the stored node carries no judicial facts and no heading.
    pub case_name: Option<String>,
    /// What kind of writing it is, in the publisher's vocabulary, such as
    /// `010combined` or `040dissent`.
    pub opinion_type: Option<String>,
    /// The judge the publisher names as its author, when it names one.
    pub author: Option<String>,
    /// The case record this writing belongs to. Two rows with one cluster are
    /// two writings in one case, such as a majority and its dissent.
    pub cluster_id: Option<u64>,
    /// The field of the publisher's record the text was read from, such as
    /// `plain_text` or `xml_harvard`, from the node's provenance
    /// (`docs/adr/0008-an-opinions-text-is-one-named-field-chosen-for-fidelity.md`).
    /// A source in another form is given whole. `None` when no provenance is
    /// recorded.
    pub text_field: Option<String>,
    /// The court that filed it. Always `None` today: the stored facts hold the
    /// docket and not the court, because the court is one more request per case
    /// (#53). A reader says "not recorded" and fetches nothing.
    pub court: Option<String>,
    /// How many `judicial.cites` links name this opinion as their subject: the
    /// U.S. Code citations the dataset holds for it.
    pub cites: usize,
}

/// Every opinion the dataset holds, one row for each expression of a judicial
/// work, ordered by date, then by cluster, then by work.
///
/// A judicial work is found by the first segment of its path, which names the
/// document class (`docs/adr/0006-a-document-node-is-class-neutral.md`).
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
            let data = &expression.root.data;
            let facts = OpinionFacts::of(data).unwrap_or_default();
            rows.push(HeldOpinion {
                work: work.to_string(),
                date: info.id.at.clone(),
                case_name: Some(facts.case_name)
                    .filter(|name| !name.is_empty())
                    .or_else(|| data.heading.as_deref().map(str::to_string)),
                opinion_type: facts.opinion_type,
                author: facts.author,
                cluster_id: facts.cluster_id,
                text_field: data
                    .provenance
                    .as_ref()
                    .map(|provenance| text_field_of(&provenance.source)),
                court: None,
                cites: cites.get(work.as_str()).copied().unwrap_or(0),
            });
        }
    }
    // The writings of one case share a cluster and its filing date, so this
    // order keeps them next to each other, and the list reads oldest first.
    rows.sort_by(|a, b| (&a.date, a.cluster_id, &a.work).cmp(&(&b.date, b.cluster_id, &b.work)));
    Ok(rows)
}

/// How many `judicial.cites` links each path is the subject of, read once.
///
/// An opinion is one node at its work's path, so the subject of a citation it
/// makes is that path.
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
