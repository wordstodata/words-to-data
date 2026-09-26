//! Reviews: what a reviewer said about a link, and which record a reader
//! reports.
//!
//! A dataset can hold a link that is wrong. A review says so, and it says so as
//! **its own link**: the link reviewed is never touched, because the statement
//! it makes is a record too
//! (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
//!
//! The shape is chosen so that two reviewers of one link are two records:
//!
//! | part | value |
//! | --- | --- |
//! | subject | the reviewed link's own subject, copied. A **locator** — it puts a review beside the link it reviews in `links_for_path` and `links_for_pair`, with no new query and no new column. |
//! | kind | `review.confirmed`, `review.refuted`, `review.disputed`. Hashed, so a reviewer who changes their mind leaves both records. |
//! | object | `External { reference: "review.argument:<link-id>:<reviewer>" }`. The **identity**. |
//!
//! The reviewer must sit in one of the three hashed parts, and that is the part
//! of this shape which surprises. A provenance is deliberately not hashed, so
//! two reviewers named only there mint one id — and `add_link`, finding the
//! first review's human-touched provenance, returns `Ok(())` and drops the
//! second. The rule that protects a review would eat a review, quietly, and
//! only when two people bothered to disagree.
//!
//! **The newest record wins, and this module ranks nobody.** No verdict is
//! reserved to a kind of reviewer and no trust order is configured: a reviewer
//! overrides by publishing over the earlier record, and every record survives.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::dataset::DatasetError;
use crate::link::{Evidence, Link, LinkKind, Provenance, Target, VerificationState};
use crate::storage::LinkWriter;

/// How a review names the link it reviews and who reviewed it.
///
/// The half before the reviewer is a prefix an indexed query already answers,
/// which is how every review of one link is found.
const REFERENCE: &str = "review.argument:";

/// How many characters of a link id a report prints, and a reviewer types.
///
/// One constant, so widening it is a one-line change that no call site
/// duplicates. A link id is a sha256 of what the link says, so a prefix of it is
/// reproducible across a rebuild and across datasets (ADR 0004).
pub const ID_PREFIX_LENGTH: usize = 12;

/// A link id shortened to what a report prints.
pub fn short_id(id: &str) -> &str {
    &id[..id.len().min(ID_PREFIX_LENGTH)]
}

/// The verdict a reviewer passed on a link.
///
/// Any reviewer may pass any of these. Writing a verdict is mechanical, so who
/// is trusted to refute is a matter of who may run the command and not a rule
/// compiled into the program (ADR 0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The link is right.
    Confirmed,
    /// The link is wrong. Settled, and settled against it.
    Refuted,
    /// The reviewer objects, and it is not settled.
    Disputed,
}

impl Verdict {
    /// The link kind this verdict is recorded as.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Confirmed => LinkKind::REVIEW_CONFIRMED,
            Self::Refuted => LinkKind::REVIEW_REFUTED,
            Self::Disputed => LinkKind::REVIEW_DISPUTED,
        }
    }

    /// The verdict a kind records, and `None` for a kind that records none.
    pub fn of_kind(kind: &str) -> Option<Self> {
        match kind {
            LinkKind::REVIEW_CONFIRMED => Some(Self::Confirmed),
            LinkKind::REVIEW_REFUTED => Some(Self::Refuted),
            LinkKind::REVIEW_DISPUTED => Some(Self::Disputed),
            _ => None,
        }
    }
}

impl std::fmt::Display for Verdict {
    /// One word, as a report prints it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Confirmed => "confirmed",
            Self::Refuted => "refuted",
            Self::Disputed => "disputed",
        })
    }
}

/// One review of one link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub verdict: Verdict,
    /// Who reviewed, in the convention [`Provenance::source`] documents:
    /// `human:jesse`, `model:local`.
    pub reviewer: String,
    /// The reviewer's stated reason, where the record carries one.
    pub reasoning: Option<String>,
    /// When the review was made.
    ///
    /// Not optional. The newest review of a link is the one a reader reports,
    /// so a record with no timestamp could never be ordered and could never win.
    pub at: OffsetDateTime,
}

impl Review {
    /// The record this review makes about `reviewed`.
    ///
    /// The subject is copied verbatim so the review comes back beside the link
    /// it reviews. That repeats what the link id in the object already implies,
    /// and the redundancy is deliberate and one-directional, on the terms ADR
    /// 0004 sets for the promoted columns: **the reference is the identity, the
    /// subject is an index.** Nothing reads the subject to learn which link is
    /// under review.
    pub fn about(&self, reviewed: &Link) -> Link {
        let reviewed_id = reviewed.id();
        Link {
            subject: reviewed.subject.clone(),
            kind: LinkKind::new(self.verdict.kind()),
            object: Target::External {
                reference: reference(&reviewed_id, &self.reviewer),
                display: format!(
                    "{} {} link {}",
                    self.reviewer,
                    self.verdict,
                    short_id(&reviewed_id)
                ),
            },
            provenance: Provenance {
                source: self.reviewer.clone(),
                method: None,
                // A review does not write a verification state. The difference
                // between a human confirming and a machine confirming was never
                // a state — it is who said it, and that is provenance (ADR
                // 0012). The state stays what the legacy annotation mapping
                // writes, and nothing here widens the enum.
                verification: VerificationState::Asserted,
                evidence: Evidence::from_reasoning(self.reasoning.clone()),
                raw_score: None,
                timestamp: Some(self.at),
                corroboration: None,
            },
            payload: None,
        }
    }

    /// The review a record states, and `None` when the link is not a review
    /// record or carries no timestamp.
    ///
    /// A record with no timestamp is not a review a reader can report, because
    /// newest-wins cannot place it among the others. It is read as no review
    /// rather than as a review at an invented time.
    pub fn read(record: &Link) -> Option<Self> {
        Some(Self {
            verdict: Verdict::of_kind(&record.kind.0)?,
            reviewer: record.provenance.source.clone(),
            reasoning: record
                .provenance
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.reasoning.clone()),
            at: record.provenance.timestamp?,
        })
    }
}

/// The review a reader reports: the newest of the records naming one link.
///
/// Newest by the reviewer's own timestamp, not by the order a store hands the
/// records back. Where two records share a moment the reviewer's name breaks the
/// tie, so one dataset gives one answer however it is read.
///
/// **Nothing here ranks a reviewer.** A later record wins whoever wrote it: an
/// agent may override a human and a human may override an agent, because writing
/// a verdict is mechanical and who is trusted to do it is a matter of who may run
/// the command (ADR 0012).
///
/// A record with no timestamp is passed over, and every record survives in the
/// dataset either way.
pub fn newest(records: &[Link]) -> Option<Review> {
    newest_of(records.iter())
}

/// The review a reader reports about one link, out of records that may name
/// others too.
///
/// One list of every review a dataset holds therefore answers for every link in
/// a walk, which is what keeps a report from asking the store once per hop.
pub fn newest_naming(link_id: &str, records: &[Link]) -> Option<Review> {
    newest_of(
        records
            .iter()
            .filter(|record| names(record) == Some(link_id)),
    )
}

fn newest_of<'a>(records: impl Iterator<Item = &'a Link>) -> Option<Review> {
    records.filter_map(Review::read).max_by(|left, right| {
        left.at
            .cmp(&right.at)
            .then_with(|| left.reviewer.cmp(&right.reviewer))
    })
}

/// The link a review record names, read out of its object.
///
/// `None` for a link that is not a review record. The **object** is asked and
/// never the subject: the subject is a copied locator, and reading it as the
/// truth about which link is under review is the one mistake this shape invites
/// (ADR 0012).
pub fn names(record: &Link) -> Option<&str> {
    let Target::External { reference, .. } = &record.object else {
        return None;
    };
    reference_parts(reference).map(|(link_id, _)| link_id)
}

/// How a review names the link it reviews and who reviewed it.
pub fn reference(reviewed_id: &str, reviewer: &str) -> String {
    format!("{REFERENCE}{reviewed_id}:{reviewer}")
}

/// The link id and the reviewer a reference names, read back out of it.
///
/// The inverse of [`reference`], and here beside it so the one format is written
/// down once. `None` for anything that is not a review reference.
///
/// ```
/// use words_to_data::review::{reference, reference_parts};
///
/// let named = reference("a1b2c3", "human:jesse");
/// assert_eq!(reference_parts(&named), Some(("a1b2c3", "human:jesse")));
/// assert_eq!(reference_parts("legislature.amendment:119-hr-1:a92dddd3"), None);
/// ```
pub fn reference_parts(reference: &str) -> Option<(&str, &str)> {
    reference.strip_prefix(REFERENCE)?.split_once(':')
}

/// The object prefix that names every review of one link.
///
/// Ends at the separator after the id, so a review of link `a1b2` is never
/// returned as a review of link `a1`.
pub fn reference_prefix(reviewed_id: &str) -> String {
    format!("{REFERENCE}{reviewed_id}:")
}

/// Why a review was not recorded.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    #[error(
        "A review record must carry a timestamp. The newest review of a link is the one a \
         reader reports, so a record with no timestamp could never be ordered and no reader \
         would ever see it."
    )]
    NoTimestamp,

    #[error(
        "A review is a link of kind `review.confirmed`, `review.refuted` or `review.disputed`, \
         and this one is `{0}`."
    )]
    NotAReview(String),

    #[error(
        "A review names the link it reviews in its object, as \
         `review.argument:<link-id>:<reviewer>`, and this one does not."
    )]
    NamesNoLink,

    #[error(transparent)]
    Store(#[from] DatasetError),
}

/// Record one review of a link.
///
/// The one door a review goes through, and the only place a malformed review is
/// caught. It writes with [`LinkWriter::add_link`] like everything else: there is
/// no second write method and a review is not a special row. A record is never
/// removed either (`docs/adr/0005-evidence-is-stored-once-and-never-deleted.md`),
/// so a wrong verdict is corrected by publishing over it.
pub fn record<S: LinkWriter>(store: &mut S, review: Link) -> Result<(), Refused> {
    if Verdict::of_kind(&review.kind.0).is_none() {
        return Err(Refused::NotAReview(review.kind.0.clone()));
    }
    let Target::External { reference, .. } = &review.object else {
        return Err(Refused::NamesNoLink);
    };
    if reference_parts(reference).is_none() {
        return Err(Refused::NamesNoLink);
    }
    if review.provenance.timestamp.is_none() {
        return Err(Refused::NoTimestamp);
    }
    store.add_link(review)?;
    Ok(())
}
