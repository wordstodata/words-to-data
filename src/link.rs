//! Links: the statements that connect a provision to something else.
//!
//! A link is core, not part of any extension, so a reader that does not know
//! an extension can still see a link, report it, and preserve it when it writes
//! the file again. Silent loss is the one failure a portable format cannot have
//! (`docs/adr/0002-links-live-in-the-core.md`).
//!
//! The kind is a namespaced string rather than an enum we control, so another
//! party can add a link type without our permission. `legislature.amended_by`
//! is ours. `judicial.cites` is ours. `westlaw.headnote` would not be.
//!
//! This module is additive today: [`ChangeAnnotation`] remains the stored form,
//! and [`Link::from_annotation`] projects it into this shape. The storage
//! migration is separate work.

use serde::{Deserialize, Serialize};

use crate::annotation::{AnnotationStatus, ChangeAnnotation};
use crate::diff::AmendmentSimilarity;

/// The kind of a link, namespaced by the extension that defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkKind(pub String);

impl LinkKind {
    /// A change to the law, caused by an amendment in a bill.
    pub const AMENDED_BY: &'static str = "legislature.amended_by";

    /// An opinion citing a provision.
    pub const CITES: &'static str = "judicial.cites";

    pub fn new(kind: impl Into<String>) -> Self {
        Self(kind.into())
    }

    /// The namespace half, `legislature` in `legislature.amended_by`.
    ///
    /// A reader uses this to decide whether it understands a link. It may not,
    /// and that is allowed: it still carries the link.
    pub fn namespace(&self) -> &str {
        self.0.split_once('.').map_or(&self.0, |(before, _)| before)
    }
}

/// What a link points at.
///
/// A closed set on purpose. A reader that does not know the legislature
/// extension can still report "this change was caused by something, and here is
/// its name", and can still tell a reference inside the dataset from one that
/// leaves it, because only the first can be checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// A provision in this dataset, by structural path.
    ///
    /// The path locates rather than identifies, so this moves to a stable
    /// provision identity when one exists
    /// (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    Provision(String),
    /// A provision as it read on one date.
    Expression { path: String, date: String },
    /// Something outside the core model, named in an extension's namespace.
    /// An amendment is reached this way, because amendments are legislature.
    External { reference: String, display: String },
}

/// How much trust a statement has earned.
///
/// Not a confidence number. A raw model score is not calibrated, so publishing
/// one as a probability claims a precision we do not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    /// A source says so.
    Asserted,
    /// A machine proposed it, and no human has confirmed it.
    MachineSuggested,
    /// A human confirmed it.
    HumanConfirmed,
    /// Contested. Someone disagrees, and it is not settled.
    Disputed,
    /// Found to be wrong. Settled, and settled against the statement.
    ///
    /// Distinct from `Disputed` on purpose: "we checked and it is false" is a
    /// stronger claim than "someone objects", and a reader deciding whether to
    /// rely on a link needs to tell them apart.
    Refuted,
}

/// Where a statement came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Who or what made the statement: `model:local`, `human:jesse`.
    pub source: String,
    /// How it was made, when that is known and worth recording.
    pub method: Option<String>,
    /// How far the statement can be trusted.
    pub verification: VerificationState,
    /// What the statement was based on.
    ///
    /// `None` for everything produced so far: raw model replies were never
    /// persisted (#58), so the evidence behind those claims is gone.
    pub evidence: Option<String>,
    /// The raw score a model reported, kept as diagnostic data only. It is not
    /// a probability and must not be presented as one.
    ///
    /// Nobody can check it. The model asserted it about its own work, and
    /// running the model again may give a different number.
    pub raw_score: Option<f32>,
    /// A deterministic measurement supporting the statement.
    ///
    /// Unlike `raw_score`, this is reproducible: a receiver holding the same
    /// texts can recompute it and get the same answer. That makes it evidence
    /// rather than a claim, and it is the one number in this struct a reader
    /// may reasonably rely on.
    ///
    /// It does not raise the verification state. A machine's proposal that
    /// scores well is still a machine's proposal; corroboration tells a human
    /// reviewer where to look first, and nothing more.
    pub corroboration: Option<Corroboration>,
}

/// A reproducible measurement that supports a statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corroboration {
    /// What was computed, so a receiver knows how to reproduce it.
    pub method: String,
    /// The headline figure, on whatever scale `method` defines.
    pub score: f32,
    /// The parts it was built from, so the figure can be checked rather than
    /// taken on trust.
    pub detail: Vec<(String, f32)>,
}

impl From<&AmendmentSimilarity> for Corroboration {
    /// Corroborate an amendment match with the deterministic overlap between
    /// the amendment's words and the words that actually changed.
    fn from(similarity: &AmendmentSimilarity) -> Self {
        Self {
            method: "precision_weighted_f1".to_string(),
            score: similarity.score,
            detail: vec![
                ("precision".to_string(), similarity.precision),
                ("recall".to_string(), similarity.recall),
                ("matched_words".to_string(), similarity.matched_words as f32),
                (
                    "tree_diff_words".to_string(),
                    similarity.tree_diff_words as f32,
                ),
            ],
        }
    }
}

/// A statement connecting a provision to something else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub subject: Target,
    pub kind: LinkKind,
    pub object: Target,
    pub provenance: Provenance,
}

impl Link {
    /// Project a stored annotation into links, one per path it covers.
    ///
    /// An annotation names several paths when one amendment changed several
    /// provisions. Each is its own statement, because each can be confirmed or
    /// disputed on its own.
    pub fn from_annotation(annotation: &ChangeAnnotation) -> Vec<Link> {
        let provenance = Provenance {
            source: annotation.metadata.annotator.clone(),
            method: Some(format!("{:?}", annotation.operation)),
            verification: verification_of(annotation),
            evidence: None,
            raw_score: annotation.metadata.confidence,
            corroboration: None,
        };

        annotation
            .paths
            .iter()
            .map(|path| Link {
                subject: Target::Provision(path.clone()),
                kind: LinkKind::new(LinkKind::AMENDED_BY),
                object: Target::External {
                    reference: format!(
                        "legislature.amendment:{}",
                        annotation.source_bill.amendment_id
                    ),
                    display: annotation.source_bill.causative_text.clone(),
                },
                provenance: provenance.clone(),
            })
            .collect()
    }

    /// Attach a reproducible measurement supporting this link.
    ///
    /// Deliberately does not change the verification state. Corroboration is
    /// evidence for a reviewer, not a substitute for one.
    pub fn with_corroboration(mut self, corroboration: Corroboration) -> Self {
        self.provenance.corroboration = Some(corroboration);
        self
    }
}

/// Map a stored status onto a verification state.
///
/// `Pending` is not a trust level, it is the absence of review, so the maker
/// decides: a machine's unreviewed claim is `MachineSuggested`, a person's is
/// `Asserted`.
///
/// `Rejected` maps to `Refuted` rather than `Disputed`: the stored status means
/// the claim was checked and found wrong, which is settled, while `Disputed`
/// means someone objects and it is not.
fn verification_of(annotation: &ChangeAnnotation) -> VerificationState {
    match annotation.metadata.status {
        AnnotationStatus::Verified => VerificationState::HumanConfirmed,
        AnnotationStatus::Disputed => VerificationState::Disputed,
        AnnotationStatus::Rejected => VerificationState::Refuted,
        AnnotationStatus::Pending => {
            if annotation.metadata.annotator.starts_with("model:") {
                VerificationState::MachineSuggested
            } else {
                VerificationState::Asserted
            }
        }
    }
}
