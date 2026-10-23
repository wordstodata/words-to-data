//! Carry the reviews of citation links across a rebuild (#319).
//!
//! A link id is a hash of the link's subject, kind and object
//! (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`), and a
//! review names the link it reviews by that id
//! (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
//! ADR 0014 moves the subject of every `judicial.cites` link from the opinion to
//! the case's writing, then to the paragraph, and its object from the section to
//! the subdivision. So each rebuild gives every citation link a new id, and a
//! review in the old dataset names nothing in the new one.
//!
//! Decision 9 of
//! `docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`
//! is the remedy, and this module is it:
//!
//! 1. **A map.** Each old citation link maps to the new links that say the same
//!    thing: the same opinion record, the same section, and the same written
//!    form. That is a [`Statement`], read from the link and not from its id, so
//!    the map does not depend on where a path puts the citing words.
//! 2. **A carried review.** Each review of an old link becomes a review of each
//!    new link it maps to: the same verdict, reviewer, reason and time, so the
//!    newest review still wins on the new link. Its payload names the old link
//!    and the old review ([`Carried`]), and its method names this carry.
//! 3. **A report.** A review whose old link maps to no new link is reported,
//!    never dropped. It stays in the old dataset, where it was written.
//!
//! This is the first case of #295, which asks for the same for every record a
//! person or an agent writes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::dataset::DatasetError;
use crate::judicial::CaseFacts;
use crate::link::{KindPayload, Link, LinkKind, Target};
use crate::method::Method;
use crate::review::{self, Refused, Review, Verdict};
use crate::storage::{DocumentReader, LinkReader, Storage};

/// The carry, named, at the version it is at now. Raise the version when the
/// map gives other answers.
pub fn carry_method() -> Method {
    Method::new("citation-review-carry", 1)
}

/// What a citation link says, apart from where the paths put it.
///
/// Two links say the same thing when they name the same opinion record, the
/// same section and the same written form. The subject path and the object path
/// can change in a rebuild, and these three cannot.
///
/// **Keep this in step with `cites_link`.** When the shape of a citation link
/// changes (#316 moves the subject to the paragraph and the object to the
/// subdivision, and puts each place the court wrote it in the payload), this is
/// the one place that must read the new shape too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Statement {
    /// The CourtListener opinion record that holds the words: `109019`, or
    /// `2812209-1` for a writing a rule found inside a combined record.
    pub record: String,
    /// The case the writing is part of, `judicial/case_<cluster>`, when the
    /// subject names one. A link made before #313 names no case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    /// The section cited, as its title's work and its section segment:
    /// `uscode/title_26/section_174`.
    pub section: String,
    /// The forms the court wrote, as the payload holds them: `174(a)(1)`.
    pub written: Vec<String>,
}

impl Statement {
    /// What a `judicial.cites` link says, or `None` for any other link.
    pub fn of(link: &Link) -> Option<Self> {
        if link.kind.0 != LinkKind::CITES {
            return None;
        }
        let record = match &link.subject {
            Target::External { reference, .. } => {
                reference.strip_prefix("judicial.opinion:")?.to_string()
            }
            subject => subject
                .path()?
                .split('/')
                .find_map(|segment| segment.strip_prefix("opinion_"))?
                .to_string(),
        };
        let case = link.subject.path().and_then(|path| {
            let mut segments = path.split('/');
            let class = segments.next()?;
            let case = segments
                .next()
                .filter(|segment| segment.starts_with("case_"))?;
            Some(format!("{class}/{case}"))
        });
        let object = link.object.path()?;
        let mut segments = object.split('/');
        let title = format!("{}/{}", segments.next()?, segments.next()?);
        let section = segments.find(|segment| segment.starts_with("section_"))?;
        let written = link
            .payload
            .as_ref()
            .and_then(|payload| payload.value["section"].as_str())
            .map(|form| vec![form.to_string()])
            .unwrap_or_default();
        Some(Self {
            record,
            case,
            section: format!("{title}/{section}"),
            written,
        })
    }

    /// Whether `new` says what this old statement said.
    fn said_again_by(&self, new: &Statement, sources: &Sources) -> bool {
        self.same_words_as(new, sources)
            && self.section == new.section
            && self
                .written
                .iter()
                .any(|form| new.written.iter().any(|other| same_form(form, other)))
    }

    /// Whether the new statement's record holds the words this old statement
    /// was read from: it is the same record, or the old record is a combined
    /// record that the new case keeps only as a source, because its split
    /// records are the writings (ADR 0014, decision 2).
    fn same_words_as(&self, new: &Statement, sources: &Sources) -> bool {
        self.record == new.record
            || new
                .case
                .as_ref()
                .is_some_and(|case| sources.get(&self.record) == Some(case))
    }
}

/// The combined records each case keeps only as a source: record id to case.
type Sources = BTreeMap<String, String>;

/// The combined records that the cases in `dataset` keep only as sources.
fn sources<D: DocumentReader>(dataset: &D) -> Result<Sources, DatasetError> {
    let mut sources = Sources::new();
    for work in dataset.works()? {
        if !work.as_str().starts_with("judicial/case_") {
            continue;
        }
        let Some(latest) = dataset.expressions(&work)?.pop() else {
            continue;
        };
        let Some(expression) = dataset.get_expression(&latest.id)? else {
            continue;
        };
        for record in CaseFacts::of(&expression.root.data)
            .map(|facts| facts.combined_records)
            .unwrap_or_default()
        {
            sources.insert(record.to_string(), work.as_str().to_string());
        }
    }
    Ok(sources)
}

/// Two written forms are the same when they differ only in white space:
/// "174 (a)(1)" and "174(a)(1)".
fn same_form(one: &str, other: &str) -> bool {
    let bare = |form: &str| form.split_whitespace().collect::<String>();
    bare(one) == bare(other)
}

/// Where a carried review came from: the old link it reviewed and the old
/// review record. Kept in the carried review's payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carried {
    /// The id of the old link the review named.
    pub link: String,
    /// The id of the old review record.
    pub review: String,
}

impl Carried {
    /// Where a review record was carried from, or `None` when it was written
    /// where it is.
    pub fn read(record: &Link) -> Option<Self> {
        let payload = record.payload.as_ref()?;
        if payload.namespace != LinkKind::REVIEW {
            return None;
        }
        serde_json::from_value(payload.value.get("carried_from")?.clone()).ok()
    }

    fn payload(&self) -> KindPayload {
        KindPayload {
            namespace: LinkKind::REVIEW.to_string(),
            value: serde_json::json!({ "carried_from": self }),
        }
    }
}

/// What happened to one review of an old citation link.
#[derive(Debug, Clone, Serialize)]
pub struct CarriedReview {
    /// The id of the old review record.
    pub review: String,
    pub verdict: Verdict,
    pub reviewer: String,
    /// The old link the review named.
    pub old_link: String,
    /// What the old link said.
    pub statement: Statement,
    /// The new links the review now names. Empty when no new link says what
    /// the old link said: the review is then reported, and stays in the old
    /// dataset.
    pub new_links: Vec<String>,
    /// Why no new link says what the old link said, when none does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// What a carry did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CarryReport {
    /// How many reviews were carried to at least one new link.
    pub carried: usize,
    /// How many reviews no new link says the same thing as. Each is in
    /// `reviews`, with its reason.
    pub not_carried: usize,
    /// Each review of an old citation link, and where it went.
    pub reviews: Vec<CarriedReview>,
}

/// Why no new link says what `old` said: the new dataset holds no citation of
/// that section by those words, or holds one only in other written forms.
fn why_not_carried(old: &Statement, new_links: &[(Link, Statement)], sources: &Sources) -> String {
    let other_forms: Vec<&str> = new_links
        .iter()
        .filter(|(_, new)| old.same_words_as(new, sources) && new.section == old.section)
        .flat_map(|(_, new)| new.written.iter().map(String::as_str))
        .collect();
    if other_forms.is_empty() {
        format!(
            "the new dataset holds no citation of {} by opinion record {}",
            old.section, old.record
        )
    } else {
        format!(
            "opinion record {} cites {} in the new dataset only as {}",
            old.record,
            old.section,
            other_forms.join(", ")
        )
    }
}

/// Carry every review of a citation link in `old` to the links in `new` that
/// say the same thing, and report each one.
///
/// A carried review is recorded through [`review::record`], the one door a
/// review goes through, so a refusal there stops the carry.
pub fn carry<O: LinkReader, N: Storage>(old: &O, new: &mut N) -> Result<CarryReport, Refused> {
    let old_links = old.links_by_kind(LinkKind::CITES)?;
    let new_links: Vec<(Link, Statement)> = new
        .links_by_kind(LinkKind::CITES)?
        .into_iter()
        .filter_map(|link| Statement::of(&link).map(|statement| (link, statement)))
        .collect();

    let old_links: BTreeMap<String, Link> = old_links
        .into_iter()
        .map(|link| (link.id(), link))
        .collect();
    let sources = sources(new)?;
    let mut report = CarryReport::default();
    for record in old.links_by_namespace(LinkKind::REVIEW)? {
        let Some(said) = Review::read(&record) else {
            continue;
        };
        let Some(old_id) = review::names(&record) else {
            continue;
        };
        let Some((reviewed, statement)) = old_links
            .get(old_id)
            .and_then(|link| Statement::of(link).map(|statement| (link, statement)))
        else {
            continue;
        };
        let carried_from = Carried {
            link: reviewed.id(),
            review: record.id(),
        };
        let mut carried_to = Vec::new();
        for (target, _) in new_links
            .iter()
            .filter(|(_, new)| statement.said_again_by(new, &sources))
        {
            let mut carried = said.about(target);
            carried.provenance.method = Some(carry_method());
            carried.payload = Some(carried_from.payload());
            review::record(new, carried)?;
            carried_to.push(target.id());
        }
        let why = if carried_to.is_empty() {
            report.not_carried += 1;
            Some(why_not_carried(&statement, &new_links, &sources))
        } else {
            report.carried += 1;
            None
        };
        report.reviews.push(CarriedReview {
            review: record.id(),
            verdict: said.verdict,
            reviewer: said.reviewer,
            old_link: reviewed.id(),
            statement,
            new_links: carried_to,
            why,
        });
    }
    Ok(report)
}
