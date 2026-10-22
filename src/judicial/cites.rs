//! The `judicial.cites` links a reader shows on a path (#301).
//!
//! A citation link has two ends, and a person can ask about either of them:
//! "what does this opinion cite" names the subject, and "which opinions cite this
//! section" names the object. `path` and `annotations` show the link from both
//! ends, so neither question needs a command of its own.

use serde::Serialize;

use crate::dataset::DatasetError;
use crate::link::{Link, LinkKind, Target, VerificationState};
use crate::query::{ObjectPlace, PathMatch, ReviewStatus, links_by_object};
use crate::review::{Review, newest_naming, short_id};
use crate::storage::LinkReader;

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
    /// The opinion that cites: the path of the node the dataset holds it as,
    /// or, for an opinion the dataset does not hold, its reference, such as
    /// `judicial.opinion:2651100`.
    pub opinion: String,
    /// Whether the link names the opinion as a node in this dataset. `false`
    /// means the citing text is outside the file, and nothing about it can be
    /// checked here (`CONTEXT.md`, "Citation").
    pub opinion_held: bool,
    /// The provision the opinion cites: the link's object.
    pub cited_path: String,
    /// The words the rule matched in the opinion, such as `26 U.S.C. § 174(e)`.
    /// From the link's evidence. `None` when the link carries no evidence.
    pub matched_words: Option<String>,
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
/// cited path and then by the opinion.
///
/// `matching` decides which paths name it, as it does for annotations: the
/// path and every path beneath it, or the path alone.
pub fn citations_naming<R: LinkReader + ?Sized>(
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
    Ok(rows)
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
        opinion_held: matches!(link.subject, Target::Node(_)),
        cited_path: link.object.path()?.to_string(),
        matched_words: link
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.clone()),
        verification: link.provenance.verification,
        review_state: review.as_ref().map_or(ReviewStatus::Unreviewed, |review| {
            ReviewStatus::of_verdict(review.verdict)
        }),
        review,
    })
}
