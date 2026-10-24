//! The `judicial.cites` links a reader shows on a path (#301).
//!
//! A citation link has two ends, and a person can ask about either of them:
//! "what does this opinion cite" names the subject, and "which opinions cite this
//! section" names the object. `path` and `annotations` show the link from both
//! ends, so neither question needs a command of its own.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::citation::resolve::stub_heading;
use crate::dataset::{DatasetError, ExpressionId};
use crate::link::{Link, LinkKind, Target, VerificationState};
use crate::query::{ObjectPlace, PathMatch, ReviewStatus, links_by_object};
use crate::review::{Review, newest_naming, short_id};
use crate::storage::{DocumentReader, LinkReader};
use crate::uslm::path::covers_path;

/// How a citation link names the path a reader asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathRole {
    /// The path is the citing opinion, or holds it: the link's subject.
    CitingOpinion,
    /// The path is the cited provision: the link's object.
    CitedPath,
    /// The cited provision is beneath the path.
    CitedBeneathPath,
    /// The cited provision holds the path: it is above it. The opinion cites
    /// that provision, and the link says nothing about the path itself.
    CitedAbovePath,
}

/// One `judicial.cites` link, as a reader of one path reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitationLink {
    /// The link's id, shortened as a report prints it. `settle` takes it.
    pub id: String,
    /// How the link names the path asked about.
    pub role: PathRole,
    /// The passage that cites: the paragraph or footnote that holds the
    /// citation, such as `judicial/case_122262/opinion_9434365/page_461/paragraph_1`
    /// (#316), or, for an opinion the dataset does not hold, its reference,
    /// such as `judicial.opinion:2651100`.
    pub opinion: String,
    /// The writing that holds the passage, `judicial/case_<cluster>/opinion_<id>`,
    /// or the reference of an opinion the dataset does not hold.
    pub writing: String,
    /// Whether the link names the opinion as a node in this dataset. `false`
    /// means the citing text is outside the file, and nothing about it can be
    /// checked here (`CONTEXT.md`, "Citation").
    pub opinion_held: bool,
    /// The provision the opinion cites: the link's object.
    pub cited_path: String,
    /// The heading of the cited provision when the latest printing held prints
    /// it as a stub, such as `[Renumbered § 30]`, and `None` for a provision of
    /// law (`crate::citation::resolve::stub_heading`, #310).
    pub stub_heading: Option<String>,
    /// The words the rule matched in the opinion, such as `26 U.S.C. § 174(e)`.
    /// From the link's evidence. `None` when the link carries no evidence.
    /// When the rule repaired an OCR form to read them, the repair follows the
    /// words: `26 V.S.C. §§ 1361(a)(1), 1363(a) (OCR repair: "V.S.C." read as
    /// "U.S.C.")`.
    pub matched_words: Option<String>,
    /// The written designations that lead to no provision in the printing the
    /// link was made against, such as `(e)` of `174(e)`, from the link's
    /// payload. The link ends at the deepest part that does (#316). Empty when
    /// every designation leads to a provision.
    pub unresolved: Vec<String>,
    /// What the link's maker recorded. A rule made every citation, so this is
    /// `machine_suggested`.
    pub verification: VerificationState,
    /// The verdict of the newest review of the link, or `unreviewed`.
    pub review_state: ReviewStatus,
    /// That review: who said it, when and why. `None` when nobody has
    /// reviewed the link.
    pub review: Option<Review>,
}

/// Every `judicial.cites` link that names `path` at either end, ordered by the
/// cited path and then by the passage. A subdivision's path follows its
/// section's, so the links of one section are together.
///
/// `matching` decides which paths name it, as it does for annotations: the
/// path and every path beneath it, or the path alone.
pub fn citations_naming<R: DocumentReader + LinkReader + ?Sized>(
    reader: &R,
    path: &str,
    matching: PathMatch,
) -> Result<Vec<CitationLink>, DatasetError> {
    // Every review, read once, so a report of many links asks the store once.
    let reviews = reader.links_by_namespace(LinkKind::REVIEW)?;
    let mut rows: Vec<CitationLink> = Vec::new();
    for link in reader.links_by_kind(LinkKind::CITES)? {
        let cites = link
            .subject
            .path()
            .is_some_and(|opinion| matching.accepts(path, opinion));
        if cites && let Some(row) = row_of(&link, PathRole::CitingOpinion, &reviews) {
            rows.push(row);
        }
    }
    for (link, place) in links_by_object(reader, LinkKind::CITES, path, matching)? {
        let role = match place {
            ObjectPlace::AtPath => PathRole::CitedPath,
            ObjectPlace::BeneathPath => PathRole::CitedBeneathPath,
            ObjectPlace::AbovePath => PathRole::CitedAbovePath,
        };
        if let Some(row) = row_of(&link, role, &reviews) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| (&a.cited_path, &a.opinion).cmp(&(&b.cited_path, &b.opinion)));

    let cited: BTreeSet<&str> = rows.iter().map(|row| row.cited_path.as_str()).collect();
    let stubs = stub_headings(reader, &cited)?;
    for row in &mut rows {
        row.stub_heading = stubs.get(&row.cited_path).cloned();
    }
    Ok(rows)
}

/// The stub heading of every cited path that is a stub, read from the latest
/// printing held of the work that holds it.
///
/// Each work is read once, however many of its paths are cited: a title of the
/// Code is tens of thousands of nodes.
fn stub_headings<R: DocumentReader + ?Sized>(
    reader: &R,
    cited: &BTreeSet<&str>,
) -> Result<BTreeMap<String, String>, DatasetError> {
    let mut stubs = BTreeMap::new();
    for work in reader.works()? {
        let paths: Vec<&str> = cited
            .iter()
            .copied()
            .filter(|path| covers_path(work.as_str(), path))
            .collect();
        if paths.is_empty() {
            continue;
        }
        let Some(latest) = reader.expressions(&work)?.pop() else {
            continue;
        };
        let id = ExpressionId::new(work, latest.id.at);
        let Some(printing) = reader.get_expression(&id)? else {
            continue;
        };
        for path in paths {
            if let Some(heading) = printing.root.find(path).and_then(stub_heading) {
                stubs.insert(path.to_string(), heading);
            }
        }
    }
    Ok(stubs)
}

/// One link, read back into the row a reader prints, with the newest of
/// `reviews` that names it. `None` for a link whose object names no path.
fn row_of(link: &Link, role: PathRole, reviews: &[Link]) -> Option<CitationLink> {
    let id = link.id();
    let review = newest_naming(&id, reviews);
    Some(CitationLink {
        id: short_id(&id).to_string(),
        role,
        opinion: link.subject.name(),
        writing: super::writing_of(&link.subject.name()).to_string(),
        opinion_held: matches!(link.subject, Target::Node(_)),
        cited_path: link.object.path()?.to_string(),
        // Read for every row at once, by `citations_naming`.
        stub_heading: None,
        matched_words: link
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.clone()),
        unresolved: unresolved_of(link),
        verification: link.provenance.verification,
        review_state: review.as_ref().map_or(ReviewStatus::Unreviewed, |review| {
            ReviewStatus::of_verdict(review.verdict)
        }),
        review,
    })
}

/// The part of what the court wrote that leads to no provision, at each place
/// in the link's payload, each once.
fn unresolved_of(link: &Link) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let places = link
        .payload
        .as_ref()
        .and_then(|payload| payload.value["occurrences"].as_array());
    for part in places
        .into_iter()
        .flatten()
        .filter_map(|place| place["unresolved"].as_str())
    {
        if !parts.iter().any(|known| known == part) {
            parts.push(part.to_string());
        }
    }
    parts
}
