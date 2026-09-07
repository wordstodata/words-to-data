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
//! and [`Link::from_annotation`] projects it into this shape. Giving links
//! their own storage — so that a kind we do not own can be carried — is
//! separate work.
//!
//! A link's targets name the same things storage does. [`Target::Expression`]
//! holds an [`ExpressionId`], the key an expression is actually stored under,
//! so a link cannot point at something no reader could resolve.

use serde::{Deserialize, Serialize};

use crate::annotation::{AnnotationStatus, ChangeAnnotation};
use crate::dataset::{ExpressionId, WorkId};
use crate::diff::AmendmentSimilarity;

/// The kind of a link, namespaced by the extension that defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkKind(pub String);

impl LinkKind {
    /// The namespace the legislature extension defines. A dataset declaring it
    /// carries legislative material, whether or not any has arrived yet.
    pub const LEGISLATURE: &'static str = "legislature";

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
    /// A work as it read on one date.
    ///
    /// The same [`ExpressionId`] storage keys on, so a link points at a thing
    /// the dataset can actually be asked for. Two spellings of one concept
    /// would let a link name something no reader could resolve.
    Expression(ExpressionId),
    /// A provision as it changed between two dates of one work.
    ///
    /// A bare provision cannot say *when* it was amended, so a link whose
    /// subject was one could not answer "what changed between these two
    /// expressions" — the question coverage and validation are built on.
    ///
    /// One work and two dates rather than two [`ExpressionId`]s: two copies of
    /// one work can disagree, and after an edit one of them will
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    Change {
        work: WorkId,
        path: String,
        from_date: String,
        to_date: String,
    },
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

impl VerificationState {
    /// Whether a person has passed judgement on this statement.
    ///
    /// A machine restating a fact must not overwrite one of these. Re-running
    /// the pipeline would otherwise destroy human review, and the loss is
    /// invisible until somebody looks for a confirmation that is gone
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    pub fn is_human_touched(self) -> bool {
        matches!(self, Self::HumanConfirmed | Self::Disputed | Self::Refuted)
    }
}

/// What a statement was based on.
///
/// A machine's claim is checkable only if a receiving party can see what the
/// machine actually said. The reasoning explains the claim; the reply is what
/// lets someone confirm the reasoning was parsed out of it faithfully. Without
/// the reply, "never lie" rests on a label rather than on evidence (#58).
///
/// The reply is a reference, not the text. One reply produces many statements,
/// so copying it onto each would claim each statement had its own reply, and
/// would store it many times over. The text lives once in the dataset, under
/// the hash of itself.
///
/// The prompt is hashed rather than stored. It is built from material the
/// dataset already holds, so keeping it would duplicate the file's own
/// contents; the hash still says whether the prompt that produced this reply is
/// the one this build produces now.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Evidence {
    /// Why the maker says it reached this answer, in their own words.
    ///
    /// For a machine-made annotation this is the reasoning the model gave.
    #[serde(default)]
    pub reasoning: Option<String>,
    /// The verbatim reply that produced the statement, by its id.
    ///
    /// Resolve it with `EvidenceReader::get_reply`. `None` means no reply was
    /// recorded, which is every statement made before this was built.
    #[serde(default)]
    pub reply: Option<String>,
    /// Which model answered, as configured when it ran.
    #[serde(default)]
    pub model: Option<String>,
    /// A hash of the exact prompt that was sent.
    #[serde(default)]
    pub prompt_hash: Option<String>,
}

impl Evidence {
    /// Evidence that carries only the maker's reasoning.
    ///
    /// The shape of every statement made before replies were recorded.
    pub fn from_reasoning(reasoning: Option<String>) -> Option<Self> {
        reasoning.map(|reasoning| Self {
            reasoning: Some(reasoning),
            ..Self::default()
        })
    }
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
    pub evidence: Option<Evidence>,
    /// The raw score a model reported, kept as diagnostic data only. It is not
    /// a probability and must not be presented as one.
    ///
    /// Nobody can check it. The model asserted it about its own work, and
    /// running the model again may give a different number.
    pub raw_score: Option<f32>,
    /// When the statement was made.
    ///
    /// Core provenance rather than a kind payload: every statement has a when,
    /// and a reader that cannot open the payload still needs it to judge the
    /// link. `None` means the maker did not record one.
    pub timestamp: Option<time::OffsetDateTime>,
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

/// Facts about a link that only the extension defining its kind understands.
///
/// The core stores it, hands it back unchanged, and never reads it. This is
/// ADR 0002's "a reader preserves what it does not understand" made storable.
///
/// Two rules keep it from becoming a dumping ground. The core never reads it.
/// And nothing a reader needs in order to *report* a link may live here: a
/// reader that cannot open the payload must still be able to say what the link
/// is, who said it, and how far it can be trusted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KindPayload {
    /// The namespace that owns these facts, such as `legislature`.
    pub namespace: String,
    /// The facts themselves, opaque to the core.
    pub value: serde_json::Value,
}

/// A statement connecting a provision to something else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub subject: Target,
    pub kind: LinkKind,
    pub object: Target,
    pub provenance: Provenance,
    /// Facts only this link's kind understands. `None` for a kind that needs
    /// nothing beyond the core.
    #[serde(default)]
    pub payload: Option<KindPayload>,
}

/// The id of a verbatim model reply: the hash of its own text.
///
/// Identified by what it says, like a link and like an amendment, so the same
/// reply recorded twice is one record and a rebuild is idempotent.
pub fn reply_id(reply: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(reply.as_bytes());
    hex::encode(hasher.finalize())
}

/// How an amendment is named as a link object.
///
/// Fully qualified — the bill and the amendment — so a reader that does not
/// know the legislature extension can still resolve what it points at. The
/// amendment id alone needed a separate `bill_id` column to be useful, which is
/// cruft from before the core and the extensions were separated.
pub fn amendment_reference(bill_id: &str, amendment_id: &str) -> String {
    format!("legislature.amendment:{bill_id}:{amendment_id}")
}

impl Link {
    /// What this link says, hashed: its subject, its kind, and its object.
    ///
    /// A row id is meaningless outside one file, and datasets are rebuilt
    /// rather than migrated, so an identity that does not survive a rebuild
    /// cannot be pointed at, confirmed, or deduplicated. `amendment_id` is
    /// already minted this way.
    ///
    /// Provenance is deliberately not hashed. Restating a fact updates one link
    /// rather than growing the table, which is what makes a rebuild idempotent.
    /// The cost is that two parties asserting one fact collapse into one record
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    pub fn id(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        // Hash the serialized form rather than a hand-built string: a separator
        // chosen by hand is a separator a value can contain.
        for part in [
            serde_json::to_string(&self.subject).unwrap_or_default(),
            serde_json::to_string(&self.kind).unwrap_or_default(),
            serde_json::to_string(&self.object).unwrap_or_default(),
        ] {
            hasher.update(part.as_bytes());
            hasher.update([0u8]);
        }
        hex::encode(hasher.finalize())
    }

    /// Project a stored annotation into links, one per path it covers.
    ///
    /// An annotation names several paths when one amendment changed several
    /// provisions. Each is its own statement, because each can be confirmed or
    /// disputed on its own.
    ///
    /// Takes the expression pair because the subject is a change, and a change
    /// is not addressable without the dates it happened between.
    pub fn from_annotation(
        annotation: &ChangeAnnotation,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Vec<Link> {
        let provenance = Provenance {
            source: annotation.metadata.annotator.clone(),
            method: None,
            verification: verification_of(annotation),
            evidence: Evidence::from_reasoning(annotation.metadata.reasoning.clone()),
            raw_score: annotation.metadata.confidence,
            timestamp: Some(annotation.metadata.timestamp),
            corroboration: None,
        };

        // The amending action and the free-text note are legislature concepts.
        // The action used to live in `Provenance.method` as a `Debug` string,
        // which round-tripped only because the enum carries no data.
        let payload = KindPayload {
            namespace: LinkKind::LEGISLATURE.to_string(),
            value: serde_json::json!({
                "operation": annotation.operation,
                "bill_id": annotation.source_bill.bill_id,
                "amendment_id": annotation.source_bill.amendment_id,
                "notes": annotation.metadata.notes,
            }),
        };

        annotation
            .paths
            .iter()
            .map(|path| Link {
                subject: Target::Change {
                    work: from.work.clone(),
                    path: path.clone(),
                    from_date: from.at.clone(),
                    to_date: to.at.clone(),
                },
                kind: LinkKind::new(LinkKind::AMENDED_BY),
                object: Target::External {
                    reference: amendment_reference(
                        &annotation.source_bill.bill_id,
                        &annotation.source_bill.amendment_id,
                    ),
                    display: annotation.source_bill.causative_text.clone(),
                },
                provenance: provenance.clone(),
                payload: Some(payload.clone()),
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

/// Regroup links into the annotations they came from.
///
/// The reverse of [`Link::from_annotation`], and the direction that matters now
/// that links are what is stored.
///
/// Links group by amendment, expression pair, and source. Dropping the source
/// would merge two annotators' accounts of one amendment into a single record
/// with one provenance, which loses who said what. Several amendments can cause
/// one change, so the amendment cannot be dropped either.
///
/// A link of another kind is skipped: this is a legislature-shaped view, and a
/// `judicial.cites` link is not an annotation.
pub fn annotations_from_links(links: &[Link]) -> Vec<ChangeAnnotation> {
    use crate::annotation::{AnnotationMetadata, BillReference};
    use std::collections::BTreeMap;

    let amended_by = LinkKind::new(LinkKind::AMENDED_BY);
    let mut grouped: BTreeMap<(String, String, String, String), ChangeAnnotation> = BTreeMap::new();

    for link in links.iter().filter(|l| l.kind == amended_by) {
        let Target::Change {
            path,
            from_date,
            to_date,
            ..
        } = &link.subject
        else {
            continue;
        };
        let Target::External { reference, display } = &link.object else {
            continue;
        };
        let payload = link.payload.as_ref().map(|p| &p.value);
        let field = |name: &str| -> Option<String> {
            payload
                .and_then(|v| v.get(name))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };

        let key = (
            reference.clone(),
            from_date.clone(),
            to_date.clone(),
            link.provenance.source.clone(),
        );
        grouped
            .entry(key)
            .or_insert_with(|| ChangeAnnotation {
                operation: field("operation")
                    .and_then(|op| op.parse().ok())
                    .unwrap_or(crate::legislature::AmendingAction::Amend),
                source_bill: BillReference {
                    bill_id: field("bill_id").unwrap_or_default(),
                    amendment_id: field("amendment_id").unwrap_or_default(),
                    causative_text: display.clone(),
                },
                paths: Vec::new(),
                metadata: AnnotationMetadata {
                    status: status_of(link.provenance.verification),
                    confidence: link.provenance.raw_score,
                    annotator: link.provenance.source.clone(),
                    timestamp: link
                        .provenance
                        .timestamp
                        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH),
                    notes: field("notes"),
                    reasoning: link
                        .provenance
                        .evidence
                        .as_ref()
                        .and_then(|e| e.reasoning.clone()),
                },
            })
            .paths
            .push(path.clone());
    }

    grouped.into_values().collect()
}

/// Map a verification state back onto a stored status.
///
/// The reverse of [`verification_of`], and lossy in one place: `Asserted` and
/// `MachineSuggested` both came from `Pending`, and both go back to it.
fn status_of(verification: VerificationState) -> AnnotationStatus {
    match verification {
        VerificationState::HumanConfirmed => AnnotationStatus::Verified,
        VerificationState::Disputed => AnnotationStatus::Disputed,
        VerificationState::Refuted => AnnotationStatus::Rejected,
        VerificationState::Asserted | VerificationState::MachineSuggested => {
            AnnotationStatus::Pending
        }
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
