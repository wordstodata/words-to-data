//! Every amendment of a public law that no standing `legislature.amended_by`
//! link names, with the stage and the reason it stopped (#251), and every
//! amendment that is linked in part (#271).
//!
//! Stage 5 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//! This is the list an agent works from.
//!
//! **An amendment is unlinked when no standing link names it, from any
//! source.** A link names an amendment by its object reference,
//! `legislature.amendment:<bill>:<amendment id>`
//! ([`crate::link::amendment_reference`]). The batch, the older model route and
//! the agent door all write that reference, so one standing link from any of
//! them takes the amendment off the list.
//!
//! **A link stands unless its newest review refutes it** (#268). The newest
//! review wins, whoever wrote it
//! (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
//! A refutation says that a link is wrong. It does not say where the
//! amendment's change is, so an amendment whose every link is refuted is work
//! again, and its row names each refuted link with the reviewer and the
//! reason. A newer review that confirms the link takes it off the list again.
//!
//! **An amendment is linked in part when a standing link names it and a change
//! under its address is claimed by no link** (#271). The address and the
//! window are the matcher's. A change is claimed when a standing `amended_by`
//! link, for any amendment, names it in that window. The row lists each change
//! that is not claimed, and says no more: that the change is under the address
//! and unclaimed. It does not say that the change is the amendment's. An agent
//! reads the words and decides. A change whose link to this amendment was
//! refuted is not listed again, because a reviewer has said that it is not
//! this amendment's.
//!
//! **What it knows comes from the matcher.** Each unlinked amendment is
//! described by the answer [`crate::legislature::evidence_matching`] gives for
//! it: the stage, the reason, the window and the changes under the address.
//! Nothing here matches anything again.
//!
//! **Not every row is work.** A row falls in one [`Category`]: work for an
//! agent, quiet, not held by the dataset, linked by the method with the link
//! not yet written, or reviewed by someone who concluded it has no link.
//!
//! **A "no link" conclusion is a record, and the list is not.** A reviewer who
//! finds that an amendment has no correct link records a
//! [`crate::review::NoLink`]. The newest such record for an amendment takes it
//! out of the work, and the row still shows who concluded it and why. It takes
//! precedence over refuted links. A link that names the amendment later takes
//! it off the list, as any standing link does.
//!
//! **Nothing is stored.** The list is derived every time it is asked for,
//! because a stored list goes false the moment someone links an amendment
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use time::OffsetDateTime;

use crate::dataset::{Dataset, DatasetError, ExpressionId};
use crate::legislature::evidence_matching::{
    AddressSource, AmendmentMatch, Outcome, Stage, is_a_note, match_by_evidence,
};
use crate::legislature::redesignation::Reason;
use crate::link::{LinkKind, Target, amendment_reference, bill_reference_prefix};
use crate::olrc::law_section::{LawSection, classifications_of};
use crate::query::LinkQuery;
use crate::review::{NoLink, Verdict, newest_naming, standing_no_links};
use crate::storage::{LegislatureReader, LinkReader, Storage};
use crate::uslm::amendment_address::AmendmentAddress;

pub use crate::olrc::law_section::Classification;

/// One amendment no standing link names, or one that is linked in part, and
/// what is known about it.
#[derive(Debug, Clone, Serialize)]
pub struct Unlinked {
    /// The bill, as the dataset names it: `119-hr-1`.
    pub bill_id: String,
    /// The public law the bill became: `119-21`.
    pub public_law: String,
    /// The amendment, by the content hash its bill's node carries.
    pub amendment_id: String,
    /// The amendment's words, as the bill states them.
    pub amending_text: String,
    /// Where the amendment acts.
    pub address: AmendmentAddress,
    /// Which evidence gave the address: the bill's markup or, when it names no
    /// section, the OLRC's classification table. `amendment-addresses` reads
    /// the markup alone (#336).
    pub address_source: AddressSource,
    /// Where the amendment sits in its public law, as the OLRC writes it:
    /// `10101(b)(3)`.
    pub law_section: Option<String>,
    /// What the OLRC's classification table says about that place in the law,
    /// one entry for each row that names it. Empty when none is stored.
    pub olrc: Vec<Classification>,
    /// Whether it is work for an agent.
    pub category: Category,
    /// What the amendment changes that the dataset does not hold, when the
    /// category is [`Category::NotHeld`].
    pub not_held: Option<String>,
    /// The newest conclusion that the amendment has no correct link, when the
    /// category is [`Category::ReviewedNoLink`].
    pub no_link: Option<NoLink>,
    /// Each refuted `amended_by` link that names the amendment. Empty when no
    /// link of it is refuted.
    pub refuted: Vec<RefutedLink>,
    /// The stage of the method it stopped at. `None` when the method links it
    /// ([`Category::Unwritten`], and [`Category::LinkedInPart`] when the method
    /// made the link).
    pub stage: Option<Stage>,
    /// Why, in words a reviewer can act on.
    pub reason: String,
    /// The older expression of the window its address changed in, when one
    /// did.
    pub from: Option<ExpressionId>,
    /// The newer expression of that window.
    pub to: Option<ExpressionId>,
    /// The changes under the address in that window, in document order: where
    /// a reviewer starts to look. For [`Category::LinkedInPart`], only the
    /// changes that no standing link claims.
    pub changes: Vec<String>,
}

/// A link that names an amendment, and the refutation that is its newest
/// review.
#[derive(Debug, Clone, Serialize)]
pub struct RefutedLink {
    /// The refuted link, by its full id.
    pub link_id: String,
    /// Who refuted it.
    pub reviewer: String,
    /// Why, in the reviewer's own words, where the record carries a reason.
    pub reasoning: Option<String>,
    /// When it was refuted.
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
}

/// Whether an unlinked amendment is work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Something is left to resolve.
    Work,
    /// The address is in the Code the dataset holds and nothing under it
    /// changed after the law's enactment
    /// ([`crate::legislature::evidence_matching::Residue::quiet`]).
    Quiet,
    /// The amendment changes something the dataset does not hold, so no
    /// change the dataset holds can be its change. Not a miss.
    NotHeld,
    /// The evidence method links it, and no link is written: the batch has
    /// not run over this dataset since.
    Unwritten,
    /// A reviewer concluded it has no correct link, and recorded why
    /// ([`crate::review::NoLink`]). Not work.
    ReviewedNoLink,
    /// A standing link names it, and a change under its address in its window
    /// is claimed by no link (#271). Work: an agent decides whether each such
    /// change is the amendment's.
    LinkedInPart,
}

/// Every amendment of the public law `bill` became, or of every public law the
/// dataset holds when `bill` is `None`, that no standing `amended_by` link
/// names, and every one that is linked in part ([`Category::LinkedInPart`]).
///
/// In the order each bill states its amendments.
pub fn unlinked_amendments<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    bill: Option<&str>,
) -> Result<Vec<Unlinked>, DatasetError> {
    let reviews = dataset.links_by_namespace(LinkKind::REVIEW)?;
    let links = amendment_links(dataset, bill, &reviews)?;
    let claims = claims(dataset, &reviews)?;
    let classified = dataset.links_by_kind(LinkKind::CLASSIFIED_FROM)?;
    let no_links = standing_no_links(&dataset.links_by_kind(LinkKind::REVIEW_NO_LINK)?, &reviews);
    let found = match_by_evidence(dataset)?;
    let rows = found
        .matches
        .into_iter()
        .filter(|amendment| bill.is_none_or(|bill| bill == amendment.bill_id))
        .filter_map(|amendment| {
            let reference = amendment_reference(&amendment.bill_id, &amendment.amendment_id);
            let refuted = links.refuted.get(&reference).cloned().unwrap_or_default();
            if links.standing.contains_key(&reference) {
                return linked_in_part(amendment, &classified, refuted, &claims, &reference);
            }
            let no_link = no_links.get(&reference).cloned();
            Some(unlinked(amendment, &classified, no_link, refuted))
        })
        .collect();
    Ok(rows)
}

/// The row of an amendment a standing link names, when a change under its
/// address in its window is claimed by no standing link. `None` when every
/// such change is claimed.
///
/// A change whose link to this amendment was refuted is left out: a reviewer
/// said that it is not this amendment's change, so it is not offered again.
fn linked_in_part(
    amendment: AmendmentMatch,
    classified: &[crate::link::Link],
    refuted: Vec<RefutedLink>,
    claims: &Claims,
    reference: &str,
) -> Option<Unlinked> {
    let (stage, from, to, under_address) = match &amendment.outcome {
        Outcome::Linked(linked) => (
            None,
            linked.from.clone(),
            linked.to.clone(),
            linked.under_address.clone(),
        ),
        Outcome::Residue(residue) => (
            Some(residue.stage),
            residue.from.clone()?,
            residue.to.clone()?,
            residue.changes.clone(),
        ),
    };
    // The diff can list one path twice, when the Code prints two provisions
    // with one number. The row names each path once.
    let mut unclaimed: Vec<String> = Vec::new();
    for path in under_address {
        if !unclaimed.contains(&path) {
            unclaimed.push(path);
        }
    }
    unclaimed.retain(|path| {
        let change = ChangeAt::new(&from, &to, path);
        !claims.claimed.contains(&change)
            && !claims.refuted.contains(&(reference.to_string(), change))
    });
    if unclaimed.is_empty() {
        return None;
    }
    let mut row = unlinked(amendment, classified, None, refuted);
    row.category = Category::LinkedInPart;
    row.stage = stage;
    row.reason = format!(
        "a standing link names it, and {} change(s) under its address in its window \
         are claimed by no link",
        unclaimed.len()
    );
    row.from = Some(from);
    row.to = Some(to);
    row.changes = unclaimed;
    Some(row)
}

/// One row, from the matcher's answer for an amendment no standing link
/// names, the newest conclusion that it has no link, if a reviewer recorded
/// one, and the links that name it and were refuted.
fn unlinked(
    amendment: AmendmentMatch,
    classified: &[crate::link::Link],
    no_link: Option<NoLink>,
    refuted: Vec<RefutedLink>,
) -> Unlinked {
    let place = LawSection::of_path(&amendment.address.path);
    let olrc = place.as_ref().map_or_else(Vec::new, |place| {
        classifications_of(classified, &amendment.public_law, place)
    });
    let not_held = not_held(&amendment.address, &olrc);
    let (category, stage, reason, from, to, changes) = match amendment.outcome {
        Outcome::Residue(residue) => {
            let category = if not_held.is_some() {
                Category::NotHeld
            } else if residue.quiet {
                Category::Quiet
            } else {
                Category::Work
            };
            (
                category,
                Some(residue.stage),
                residue.reason,
                residue.from,
                residue.to,
                residue.changes,
            )
        }
        Outcome::Linked(linked) => (
            Category::Unwritten,
            None,
            "the evidence method links it, and no link is written: run link-by-evidence"
                .to_string(),
            Some(linked.from.clone()),
            Some(linked.to.clone()),
            linked.paths(),
        ),
    };
    // A reviewer's statement outranks what the matcher derives: the matcher
    // says where it stopped, and the reviewer looked further. A conclusion that
    // the amendment has no link settles it. A refutation only says that a link
    // is wrong, so the amendment is work again.
    let (category, reason) = if no_link.is_some() {
        (Category::ReviewedNoLink, reason)
    } else if !refuted.is_empty() {
        (Category::Work, "its every link was refuted".to_string())
    } else {
        (category, reason)
    };
    Unlinked {
        bill_id: amendment.bill_id,
        public_law: amendment.public_law,
        amendment_id: amendment.amendment_id,
        amending_text: amendment.amending_text,
        address: amendment.address,
        address_source: amendment.address_source,
        law_section: place.map(|place| place.to_string()),
        olrc,
        category,
        not_held,
        no_link,
        refuted,
        stage,
        reason,
        from,
        to,
        changes,
    }
}

/// What the dataset does not hold that the amendment changes, if anything.
fn not_held(address: &AmendmentAddress, olrc: &[Classification]) -> Option<String> {
    if address.unresolved == Some(Reason::TableOfSections) {
        return Some(
            "a table of sections, which the dataset does not hold as a provision".to_string(),
        );
    }
    let only_notes = !olrc.is_empty()
        && olrc
            .iter()
            .flat_map(|row| &row.descriptions)
            .all(|description| is_a_note(description));
    only_notes.then(|| {
        "the OLRC classifies this section of the law only as a note or as the heading \
         before a section, and the dataset holds neither"
            .to_string()
    })
}

/// How many standing `legislature.amended_by` links name each amendment of
/// `bill`, or of every bill when `bill` is `None`, by the amendment's object
/// reference ([`amendment_reference`]).
///
/// A link stands unless its newest review refutes it, as for the list above. An
/// amendment that no standing link names is not in the map.
pub fn standing_links<S: LinkReader + ?Sized>(
    dataset: &S,
    bill: Option<&str>,
) -> Result<BTreeMap<String, usize>, DatasetError> {
    let reviews = dataset.links_by_namespace(LinkKind::REVIEW)?;
    Ok(amendment_links(dataset, bill, &reviews)?.standing)
}

/// One change, as a link's subject names it: a path of a work between two
/// dates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ChangeAt {
    work: String,
    from: String,
    to: String,
    path: String,
}

impl ChangeAt {
    fn new(from: &ExpressionId, to: &ExpressionId, path: &str) -> Self {
        Self {
            work: from.work.to_string(),
            from: from.at.clone(),
            to: to.at.clone(),
            path: path.to_string(),
        }
    }

    /// The change a link's subject names, when it names one.
    fn of(subject: &Target) -> Option<Self> {
        let Target::Change {
            work,
            path,
            from_date,
            to_date,
        } = subject
        else {
            return None;
        };
        Some(Self {
            work: work.to_string(),
            from: from_date.clone(),
            to: to_date.clone(),
            path: path.clone(),
        })
    }
}

/// Which changes the `amended_by` links claim, from every bill.
#[derive(Default)]
struct Claims {
    /// Each change that at least one standing link names.
    claimed: BTreeSet<ChangeAt>,
    /// Each change whose link to an amendment was refuted, with that
    /// amendment's object reference.
    refuted: BTreeSet<(String, ChangeAt)>,
}

/// Every change the standing `amended_by` links claim, and every change whose
/// link to an amendment was refuted.
///
/// From every bill, because a change under one law's address can be another
/// law's change. A link stands unless its newest review refutes it, as for
/// [`amendment_links`].
fn claims<S: LinkReader + ?Sized>(
    dataset: &S,
    reviews: &[crate::link::Link],
) -> Result<Claims, DatasetError> {
    let mut claims = Claims::default();
    let query = LinkQuery::new().of_kind(LinkKind::AMENDED_BY);
    for link in dataset.links_matching(&query)?.rows {
        let Some(change) = ChangeAt::of(&link.subject) else {
            continue;
        };
        match newest_naming(&link.id(), reviews) {
            Some(review) if review.verdict == Verdict::Refuted => {
                claims.refuted.insert((link.object.name(), change));
            }
            _ => {
                claims.claimed.insert(change);
            }
        }
    }
    Ok(claims)
}

/// What the `amended_by` links say about each amendment they name, by its
/// object reference.
#[derive(Default)]
struct AmendmentLinks {
    /// Every amendment that at least one standing link names, and how many
    /// standing links name it.
    standing: BTreeMap<String, usize>,
    /// The refuted links of each amendment, whether or not a link of it
    /// stands.
    refuted: BTreeMap<String, Vec<RefutedLink>>,
}

/// Every `amended_by` link, sorted into standing and refuted.
///
/// A link stands unless its newest review, out of `reviews`, refutes it
/// (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
/// [`newest_naming`] picks that review, as it does for every reader.
fn amendment_links<S: LinkReader + ?Sized>(
    dataset: &S,
    bill: Option<&str>,
    reviews: &[crate::link::Link],
) -> Result<AmendmentLinks, DatasetError> {
    let mut query = LinkQuery::new().of_kind(LinkKind::AMENDED_BY);
    if let Some(bill) = bill {
        query = query.with_object_prefix(bill_reference_prefix(bill));
    }
    let mut links = AmendmentLinks::default();
    for link in dataset.links_matching(&query)?.rows {
        let link_id = link.id();
        match newest_naming(&link_id, reviews) {
            Some(review) if review.verdict == Verdict::Refuted => {
                links
                    .refuted
                    .entry(link.object.name())
                    .or_default()
                    .push(RefutedLink {
                        link_id,
                        reviewer: review.reviewer,
                        reasoning: review.reasoning,
                        at: review.at,
                    });
            }
            _ => {
                *links.standing.entry(link.object.name()).or_insert(0) += 1;
            }
        }
    }
    Ok(links)
}
