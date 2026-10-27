//! Each amendment of a public law, linked to the change it made from evidence
//! the dataset already holds, with no model call (#250).
//!
//! Stages 3 and 4 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`:
//!
//! 1. **Address.** The markup resolver ([`crate::uslm::amendment_address`])
//!    says which section, and which provision below it, an instruction acts
//!    on. Where it names no section, the one section the OLRC's classification
//!    gives the instruction's place in the law is the address (#259). Neither,
//!    and the amendment stops here.
//! 2. **Window.** The first window of the Code after the law's enactment date
//!    in which something under the address changed. The enactment date is the
//!    date of the law's stored expression, such as
//!    `publiclawdocument_119-21@2025-07-04`. Where nothing under the address
//!    changed in any window, but a window added or removed a provision the
//!    address is inside, that provision is the change: the diff lists it as
//!    one path and nothing beneath it (#336). The method gives it only to an
//!    amendment that only adds, and only when the provision was added.
//!    Where a later amendment of the same law renumbered the provision the
//!    address names in that window, the address is read at the new number
//!    (#434). Where another law moved it in that window, the words the
//!    amendment quotes say where it is now (#391).
//! 3. **Resolve.** The changes under one section in that window, assigned to
//!    the amendments addressed there all together, by the words the bill
//!    quotes ([`resolve`]).
//!
//! # The window rule
//!
//! A window can hold a law's change only when it ends after the law's
//! enactment date. Public Law 119-21 was enacted on 2025-07-04, and the first
//! committed release point after it is dated 2025-07-18:
//!
//! ```
//! use words_to_data::legislature::evidence_matching::window_can_hold;
//!
//! assert!(window_can_hold("2025-07-04", "2025-07-18"));
//! // A window that ends on the day of enactment, or before it, holds the Code
//! // as it read before the law, so nothing is linked in it.
//! assert!(!window_can_hold("2025-07-04", "2025-07-04"));
//! ```
//!
//! **Nothing here is stored.** The answer for each amendment, linked or
//! stopped with the stage and the reason, is derived from the dataset every
//! time it is asked for (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! A command writes the links; the reason an amendment stopped stays
//! computable for the residue report (#251).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::dataset::{Dataset, DatasetError, ExpressionId, WorkId};
use crate::diff::{FieldChangeEvent, TreeDiff};
use crate::document::{DocumentNode, NodeData};
use crate::inspect::provision_words;
use crate::legislature::AmendingAction;
use crate::legislature::redesignation::Reason;
use crate::legislature::redesignation::{SectionIndex, Step, walk_down};
use crate::link::{
    Evidence, KindPayload, Link, LinkKind, Provenance, Target, VerificationState,
    amendment_reference,
};
use crate::method::Method;
use crate::storage::{LegislatureReader, LinkReader, Storage};
use crate::uslm::UslmFacts;
use crate::uslm::amendment_address::{AmendmentAddress, addresses_in, units_named_in};

use quoted_words::{AddedWords, QuotedWords};
use resolve::{Change, Contender, Resolution, Whole, resolve};

mod code_edits;
mod olrc;
mod quoted_words;
mod recorded;

pub(crate) use olrc::is_a_note;
use olrc::olrc_address;
pub use olrc::{OlrcClassification, olrc_classification};
pub use recorded::{Chosen, Recorded, RecordedSource};
mod resolve;

/// What the matcher found for one amendment.
#[derive(Debug, Clone, Serialize)]
pub struct AmendmentMatch {
    /// The bill, as the dataset names it: `119-hr-1`.
    pub bill_id: String,
    /// The public law the bill became, by its number: `119-21`.
    pub public_law: String,
    /// The amendment, by the content hash its bill's node carries.
    pub amendment_id: String,
    /// The amendment's words, as the bill states them.
    pub amending_text: String,
    /// The action the bill's markup states for it.
    pub operation: AmendingAction,
    /// The law's enactment date: the date of its stored expression.
    pub enacted: String,
    /// Where the amendment acts: the bill's markup says so or, when it names
    /// no section, the OLRC's classification does ([`Self::address_source`]).
    pub address: AmendmentAddress,
    /// Which evidence gave the address.
    pub address_source: AddressSource,
    /// Linked, or stopped with a reason.
    pub outcome: Outcome,
}

/// Which evidence gave an amendment its address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressSource {
    /// The bill's markup names the section.
    Markup,
    /// The markup names none, and the OLRC's classification table classifies
    /// the amendment's place in the law to one section of the Code.
    Olrc {
        /// The sections of the law the table's rows name, as it writes them:
        /// `70204(a)(1)`.
        law_sections: Vec<String>,
        /// Why the markup gave no section.
        markup_gave_none: Reason,
    },
}

/// Linked to changes, or residue.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Linked(Linked),
    Residue(Residue),
}

/// The changes one amendment made, in the window it made them in.
#[derive(Debug, Clone, Serialize)]
pub struct Linked {
    /// The older expression of the window.
    pub from: ExpressionId,
    /// The newer expression of the window.
    pub to: ExpressionId,
    /// Each change the amendment caused, in document order.
    pub changes: Vec<CausedChange>,
    /// Every change under the address in the window, in document order: the
    /// changes the amendment's own were told apart from, its own among them.
    /// One means the matcher had nothing to choose between. `residue` reads
    /// the ones no link claims (#271).
    pub under_address: Vec<String>,
    /// Every later window in which something under the address changed too.
    ///
    /// Never a second link. The law landed in the first window after its
    /// enactment that shows it, and a change under the same address later is
    /// something else, or the same change seen twice. It is named so a
    /// reviewer can look (`docs/adr/0013`).
    pub later_windows: Vec<LaterWindow>,
    /// What the OLRC's classification says about the section. Evidence for a
    /// reviewer, and never a reason for the link.
    pub olrc: OlrcClassification,
}

impl Linked {
    /// The changed paths, in document order.
    pub fn paths(&self) -> Vec<String> {
        self.changes
            .iter()
            .map(|change| change.path.clone())
            .collect()
    }
}

/// One change an amendment caused, and why it was given to that amendment.
#[derive(Debug, Clone, Serialize)]
pub struct CausedChange {
    /// The changed path.
    pub path: String,
    /// How the change was told apart from the others under the address, in
    /// words a reviewer can check.
    pub why: String,
}

/// A later window in which an address changed again.
#[derive(Debug, Clone, Serialize)]
pub struct LaterWindow {
    pub from: ExpressionId,
    pub to: ExpressionId,
    /// The changes under the address in that window, in document order.
    pub changes: Vec<String>,
}

/// Why an amendment was not linked.
#[derive(Debug, Clone, Serialize)]
pub struct Residue {
    /// The stage it stopped at.
    pub stage: Stage,
    /// Why, in words a reviewer can act on.
    pub reason: String,
    /// The address is in the Code the dataset holds, and nothing under it
    /// changed in any window held after the law's enactment.
    ///
    /// Not work. A corpus that does not reach the date an amendment takes
    /// effect is the ordinary state of a growing dataset (#211), and nothing is
    /// there to resolve until a later release point is added.
    pub quiet: bool,
    /// The older expression of the window the address changed in, when one
    /// did.
    pub from: Option<ExpressionId>,
    /// The newer expression of that window.
    pub to: Option<ExpressionId>,
    /// The changes under the address in that window, in document order: where
    /// a reviewer starts to look.
    pub changes: Vec<String>,
}

/// The stage of the method at which an amendment stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The bill's markup gives no address.
    Address,
    /// No window holds a change under the address.
    Window,
    /// Changes were found under the address, and none could be given to this
    /// amendment.
    Resolve,
}

/// The reasoning this module applies, at the version it is at now.
///
/// A different reasoning from the model method `match-amendments` ran before it
/// was removed (#252), so a new name and not a new version of that one (#179,
/// decision 10). Raise the
/// version when this method's answers change — a new rule for telling changes
/// apart, a different window rule. Tidying the code that gives the same answers
/// is not such a change (`crate::method::Method`).
///
/// Version 4 (#274): elimination gives an amendment only a change its own
/// words can have made. The change must be in a unit its words name, and of a
/// kind its action makes: a strike brings no new words, and an addition removes
/// no provision.
///
/// Version 5 (#339): the address keeps every container the bill states
/// (*"is amended, in subsection (c)"*, *"Paragraph (2) of section 6213(b)"*,
/// *"Section 6213(b)(1) of the Internal Revenue Code"*). A provision placed
/// by quoted words does not carry a change that another amendment is
/// addressed to, and that amendment takes the provision at its own address by
/// elimination.
///
/// Version 6 (#364): *"Such section"* and *"Such subsection"* name the unit
/// the nearest earlier amendment in the same section of the bill named, so the
/// markup gives these amendments their address. Before, only the OLRC's table
/// could give one, and only the section.
///
/// Version 7 (#387): where the publisher's reference places a whole section of
/// an Act, the address keeps the units the line names below it:
/// *"Subsection (c)(7) of section 102A of the National Security Act of 1947
/// (50 U.S.C. 3024)"* acts in 3024(c)(7), not in the whole of § 3024.
///
/// Version 7 (#389, #390): an enacted block that a change shows by three
/// words or fewer, inside a longer block, is weak evidence. It is not counted
/// where other words show in the change, and it gives way to a provision
/// around the change that carries it. A provision carries every change inside
/// it only when its amendment enacted it anew, renumbered it, or inserted words
/// that open a new provision. A provision whose words the amendment only
/// edited carries only the removal of a provision inside it.
///
/// Version 7 (#432): a provision the window added is compared with the words
/// of every provision below it, field by field, in the order the Code prints
/// them. Its own words alone show it only when they are more than three
/// words, or when the block also holds a field below them.
///
/// Version 8 (#438): an item of a list under a line that names part of a
/// title gives the section it names after *by amending*, after the
/// publisher's marginal note, or in the `§ 310.` heading of a new section it
/// inserts: *"Chapter 73 of title 46 … is amended— (1) by amending section
/// 7306 …"* acts in § 7306.
///
/// Also in version 8 (#434): a section a line says *is repealed* is an address, where
/// the line names the Code. Where two amendments' quoted words fit a change
/// equally, the one whose address is inside the address of every other gets
/// it. An address that a later amendment of the same law renumbered in the
/// window is read at the new number.
///
/// Also in version 8 (#433): an enacted block shows a change also as the OLRC prints
/// it: *"title 5"* for *"title 5, United States Code"*, the Code's citation
/// for a reference to a law that the bill cites, and the date of enactment
/// for *"the date of the enactment of this Act"*. A single and a double
/// quotation mark are one mark.
///
/// Version 9 (#452): an enacted block shows an added provision also when the
/// Code prints two of its words with no space between them, as a table cell
/// of two lines: *"Applicablepercentage"* at 26 U.S.C. 1202(a)(5).
///
/// Version 9 (#391): where another law moved the provision at the address in
/// the window, and the dataset holds no record of that law, the address is
/// read at the provision that shows the words the amendment adds
/// ([`moved_by_another_law`]). The quoted words decide, and the number alone
/// does not.
pub fn evidence_method() -> Method {
    Method::new("address, window and quoted words", 9)
}

/// Who a link this method writes says made it.
const SOURCE: &str = "rule:evidence_matching";

impl AmendmentMatch {
    /// One `legislature.amended_by` link for each change this amendment
    /// caused, or none when it is residue.
    ///
    /// The shape `match-amendments` wrote before its removal (#252), so every reader of those links
    /// reads these unchanged: the subject is the change, the object is the
    /// amendment, and the payload is the legislature's. The address, the
    /// window and the words that placed the change are the link's evidence.
    ///
    /// `MachineSuggested`: the bill asserted the amendment, and the reading of
    /// which change it made is ours (`docs/adr/0010`). No clock reading,
    /// because the same dataset gives the same answer on any day.
    pub fn links(&self) -> Vec<Link> {
        let Outcome::Linked(linked) = &self.outcome else {
            return Vec::new();
        };
        linked
            .changes
            .iter()
            .map(|change| Link {
                subject: Target::Change {
                    work: linked.from.work.clone(),
                    path: change.path.clone(),
                    from_date: linked.from.at.clone(),
                    to_date: linked.to.at.clone(),
                },
                kind: LinkKind::new(LinkKind::AMENDED_BY),
                object: Target::External {
                    reference: amendment_reference(&self.bill_id, &self.amendment_id),
                    display: self.amending_text.clone(),
                },
                provenance: Provenance {
                    source: SOURCE.to_string(),
                    method: Some(evidence_method()),
                    verification: VerificationState::MachineSuggested,
                    evidence: Evidence::from_reasoning(Some(self.reasoning(linked, change))),
                    raw_score: None,
                    timestamp: None,
                    corroboration: None,
                },
                payload: Some(KindPayload {
                    namespace: LinkKind::LEGISLATURE.to_string(),
                    value: serde_json::json!({
                        "operation": self.operation,
                        "bill_id": self.bill_id,
                        "amendment_id": self.amendment_id,
                        "notes": null,
                    }),
                }),
            })
            .collect()
    }

    /// The link's evidence: where, when, and by which words.
    fn reasoning(&self, linked: &Linked, change: &CausedChange) -> String {
        let source = match &self.address_source {
            AddressSource::Markup => "read from the bill's markup".to_string(),
            AddressSource::Olrc {
                law_sections,
                markup_gave_none,
            } => format!(
                "read from the OLRC classification table, which classifies Pub. L. {} {} to \
                 this section; the bill's markup gives none, because {markup_gave_none}",
                self.public_law,
                law_sections
                    .iter()
                    .map(|section| format!("§ {section}"))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        };
        let mut reasoning = format!(
            "Address: {}, {source}. \
             Window: {} to {}, the first window after the law's enactment on {} \
             in which something under the address changed. ",
            address_text(&self.address),
            linked.from,
            linked.to.at,
            self.enacted
        );
        reasoning.push_str(&format!(
            "The address holds {} change(s) in this window. ",
            linked.under_address.len()
        ));
        for later in &linked.later_windows {
            reasoning.push_str(&format!(
                "The address changed again from {} to {}, and that window is not linked. ",
                later.from, later.to.at
            ));
        }
        reasoning.push_str("Change: ");
        reasoning.push_str(&change.why);
        reasoning.push(' ');
        reasoning.push_str(&linked.olrc.sentence(&self.public_law));
        reasoning
    }
}

/// What one run of the matcher found.
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceMatching {
    /// One answer for every amendment of every public law the dataset holds,
    /// in the order each bill states them.
    pub matches: Vec<AmendmentMatch>,
    /// Every window the method read, in the order it read them.
    ///
    /// A window it read and linked nothing in has still been worked on, so a
    /// run is recorded over each of these and not only over the windows that
    /// carry a link (#179, decision 11).
    pub windows: Vec<(ExpressionId, ExpressionId)>,
    /// Each public law whose amendments strike or insert quoted strings in
    /// their own words, while none of them is stored with one:
    /// `(bill_id, public_law)`.
    ///
    /// A dataset whose bills were stored before #257 kept no quoted strings.
    /// Without them the matcher links fewer of the law's amendments (408
    /// against 449 on `119-hr-1`), so a reader is told.
    pub laws_quoting_no_strings: Vec<(String, String)>,
}

/// What the matcher finds for every amendment of every public law the dataset
/// holds.
pub fn match_by_evidence<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
) -> Result<EvidenceMatching, DatasetError> {
    match_by_evidence_reporting(dataset, &crate::progress::Silent)
}

/// [`match_by_evidence`], telling `progress` as each title of the Code is
/// matched. Almost all of the time goes there, one title at a time.
pub fn match_by_evidence_reporting<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    progress: &dyn crate::progress::Progress,
) -> Result<EvidenceMatching, DatasetError> {
    match_laws(dataset, None, progress)
}

/// What the matcher finds for every amendment of the public law `bill` became,
/// in the order the bill states them.
///
/// These are the answers [`match_by_evidence`] gives for the bill, in less
/// time: only the titles of the Code that the bill's amendments act in are
/// matched (#398). Each of those titles is still matched with the amendments
/// of every law that act in it, because the amendments that act in one section
/// in one window are resolved together.
pub fn match_bill_by_evidence<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    bill: &str,
) -> Result<Vec<AmendmentMatch>, DatasetError> {
    Ok(match_laws(dataset, Some(bill), &crate::progress::Silent)?.matches)
}

/// The matcher over every public law or, when `bill` is given, over the titles
/// of the Code that the bill acts in, with answers for that bill alone.
fn match_laws<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    bill: Option<&str>,
    progress: &dyn crate::progress::Progress,
) -> Result<EvidenceMatching, DatasetError> {
    progress.begin("read the amendments", None);
    let mut stated = stated_amendments(dataset)?;
    let classifications = dataset.links_by_kind(LinkKind::CLASSIFIED_FROM)?;
    let mut outcomes: Vec<Option<Outcome>> = vec![None; stated.len()];
    let mut windows = Vec::new();

    // Stage 1: the address. Each addressed amendment is sent on to the work of
    // the Code its section sits in.
    let mut by_work: BTreeMap<WorkId, Vec<usize>> = BTreeMap::new();
    for (at, amendment) in stated.iter_mut().enumerate() {
        address_by_olrc_where_the_markup_gives_none(amendment, &classifications);
        match addressed_work(&amendment.address) {
            Ok(work) => by_work.entry(work).or_default().push(at),
            Err(reason) => outcomes[at] = Some(residue(Stage::Address, reason)),
        }
    }
    // A title the bill does not act in can change none of the bill's answers.
    if let Some(bill) = bill {
        by_work.retain(|_, members| members.iter().any(|&at| stated[at].bill_id == bill));
    }

    // One work at a time, so only one title of the Code is in memory at once.
    progress.begin("titles of the Code", Some(by_work.len() as u64));
    for (work, members) in &by_work {
        windows.extend(match_in_work(
            dataset,
            work,
            &stated,
            members,
            &classifications,
            &mut outcomes,
        )?);
        progress.advance(1);
    }

    let laws_quoting_no_strings = laws_quoting_no_strings(&stated);
    let matches = stated
        .into_iter()
        .zip(outcomes)
        .filter(|(amendment, _)| bill.is_none_or(|bill| bill == amendment.bill_id))
        .map(|(amendment, outcome)| AmendmentMatch {
            bill_id: amendment.bill_id,
            public_law: amendment.public_law,
            amendment_id: amendment.address.amendment_id.clone(),
            amending_text: amendment.amending_text,
            operation: amendment.operation,
            enacted: amendment.enacted,
            address: amendment.address,
            address_source: amendment.address_source,
            outcome: outcome.expect("every amendment is answered for"),
        })
        .collect();
    Ok(EvidenceMatching {
        matches,
        windows,
        laws_quoting_no_strings,
    })
}

/// Each public law whose amendments strike or insert quoted strings in their
/// own words, `by striking "2023"`, while none of them is stored with one, in
/// the order the dataset lists them.
///
/// A law that quotes no string at all is no fault: Pub. L. 119-22 only
/// enacts a subparagraph anew.
fn laws_quoting_no_strings(stated: &[Stated]) -> Vec<(String, String)> {
    static STRIKES_OR_INSERTS_A_STRING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)\b(striking|inserting)\s+["“]"#).expect("the pattern must compile")
    });
    let mut laws: Vec<(String, String)> = Vec::new();
    for amendment in stated {
        let law = (amendment.bill_id.clone(), amendment.public_law.clone());
        if !laws.contains(&law) {
            laws.push(law);
        }
    }
    laws.retain(|(bill_id, _)| {
        let of_law = || {
            stated
                .iter()
                .filter(|amendment| &amendment.bill_id == bill_id)
        };
        let stores_a_string = of_law().any(|amendment| amendment.evidence.quotes_strings());
        let words_quote_a_string = of_law()
            .any(|amendment| STRIKES_OR_INSERTS_A_STRING.is_match(&amendment.amending_text));
        words_quote_a_string && !stores_a_string
    });
    laws
}

/// One amendment as its public law states it.
struct Stated {
    bill_id: String,
    /// The law's number: `119-21`.
    public_law: String,
    /// The date of the law's stored expression.
    enacted: String,
    amending_text: String,
    operation: AmendingAction,
    /// Every action the markup states for it and the levels nested in it.
    actions: Vec<AmendingAction>,
    address: AmendmentAddress,
    address_source: AddressSource,
    /// The words the bill quotes for it.
    evidence: QuotedWords,
}

/// Every amendment of every public law the dataset holds as a document.
fn stated_amendments<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
) -> Result<Vec<Stated>, DatasetError> {
    let mut stated = Vec::new();
    for bill_id in dataset.list_bill_ids()? {
        // Only a public law is matched: a bill that has not been enacted
        // changes nothing (`docs/adr/0013`). A bill with no public law document
        // in the dataset is not one this dataset can match.
        let Some(law) = dataset.bill_document(&bill_id)? else {
            continue;
        };
        let Some(bill) = dataset.get_bill(&bill_id)? else {
            continue;
        };
        // The law's number is the name of its work: `publiclawdocument_119-21`.
        let public_law = crate::dataset::public_law_number(&law.id.work)
            .unwrap_or(law.id.work.as_str())
            .to_string();
        for address in addresses_in(&law.root) {
            let evidence = law
                .root
                .find(&address.path)
                .and_then(|node| UslmFacts::of(&node.data))
                .and_then(|facts| facts.amendment)
                .map(|facts| QuotedWords::of(&facts, &law.id.at))
                .unwrap_or_default();
            let amendment = bill.amendments.get(&address.amendment_id);
            stated.push(Stated {
                bill_id: bill_id.clone(),
                public_law: public_law.clone(),
                enacted: law.id.at.clone(),
                amending_text: amendment
                    .map(|amendment| amendment.amending_text.clone())
                    .unwrap_or_else(|| address.text.clone()),
                operation: amendment.map_or(AmendingAction::Amend, |amendment| {
                    stated_operation(&amendment.action_types)
                }),
                actions: amendment
                    .map(|amendment| amendment.action_types.clone())
                    .unwrap_or_default(),
                address,
                address_source: AddressSource::Markup,
                evidence,
            });
        }
    }
    Ok(stated)
}

/// Give an amendment the section the OLRC's classification gives it, when the
/// markup gives none.
///
/// **The markup wins.** An amendment the markup addresses is never changed
/// here, and neither is one that changes a table of sections: the OLRC
/// classifies the law's section that holds it, which also holds the provision
/// the table lists, so its row names a section the table amendment does not
/// act on. The scope phrases the markup did read (*"in paragraph (87)"*) stay
/// the container below the section. Each names its level, so a wrong one finds
/// no provision and links nothing.
///
/// **An amendment that quotes no words gets no address from the table.** A row
/// gives a section and nothing below it, and such an amendment has nothing to
/// tell its change from the others in the section. *"Subsection (e) of section
/// 455 … is repealed"* took a change to § 1087e(d)(5)(B), which another
/// amendment made, when the table alone addressed it (#259).
fn address_by_olrc_where_the_markup_gives_none(amendment: &mut Stated, classifications: &[Link]) {
    let Some(markup_gave_none) = amendment.address.unresolved.clone() else {
        return;
    };
    if markup_gave_none == Reason::TableOfSections || !amendment.evidence.quotes_anything() {
        return;
    }
    let Some(found) = olrc_address(
        classifications,
        &amendment.public_law,
        &amendment.address.path,
    ) else {
        return;
    };
    amendment.address.section = Some(found.section);
    amendment.address.unresolved = None;
    amendment.address_source = AddressSource::Olrc {
        law_sections: found.law_sections,
        markup_gave_none,
    };
}

/// The action a link records: the one the bill's markup states.
///
/// `amend` is the markup's umbrella word, and most instructions carry it beside
/// the action that says what they do, so it is set aside. When one action is
/// left, that is the action. When none or several are left, `amend` is the
/// honest word, because choosing one of several would claim a reading nobody
/// made.
fn stated_operation(actions: &[AmendingAction]) -> AmendingAction {
    let mut specific: Vec<AmendingAction> = Vec::new();
    for action in actions {
        if *action != AmendingAction::Amend && !specific.contains(action) {
            specific.push(*action);
        }
    }
    match specific.as_slice() {
        [one] => *one,
        _ => AmendingAction::Amend,
    }
}

/// The work of the Code an address acts in, or why there is none.
fn addressed_work(address: &AmendmentAddress) -> Result<WorkId, String> {
    let Some(section) = &address.section else {
        return Err(address.unresolved.as_ref().map_or(
            "the markup gives no address".to_string(),
            ToString::to_string,
        ));
    };
    work_of(section).ok_or_else(|| format!("{section} is not a section of the US Code"))
}

/// An address as one string: the section, then each step below it,
/// `/us/usc/t26/s11026(a)`.
fn address_text(address: &AmendmentAddress) -> String {
    let below: String = address
        .container
        .iter()
        .map(|step| format!("({})", step.number))
        .collect();
    format!("{}{below}", address.section.as_deref().unwrap_or_default())
}

/// The work of the Code a section identifier sits in: `/us/usc/t7/s2028` is in
/// `uscode/title_7`.
fn work_of(section: &str) -> Option<WorkId> {
    let title = section.strip_prefix("/us/usc/t")?.split('/').next()?;
    Some(WorkId::new(format!("uscode/title_{title}")))
}

/// One window of a work, read once for every amendment that acts in it.
struct WindowView {
    from: ExpressionId,
    to: ExpressionId,
    earlier: DocumentNode,
    later: DocumentNode,
    /// Every change in the window, in document order.
    changes: Vec<Change>,
    /// Who made each renumbering in the window.
    renumberings: Renumberings,
}

/// Where one amendment's address changed in one window.
struct Placed {
    window: usize,
    /// The section's own path, which groups the amendments resolved together.
    section: String,
    /// The path of the provision the address names, at or below `section`.
    address: String,
    /// The changes under the address, by their place in the window's list.
    candidates: Vec<usize>,
}

/// Match every amendment that acts in one work, and say which windows were
/// read.
fn match_in_work<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    work: &WorkId,
    stated: &[Stated],
    members: &[usize],
    classifications: &[Link],
    outcomes: &mut [Option<Outcome>],
) -> Result<Vec<(ExpressionId, ExpressionId)>, DatasetError> {
    let earliest_enactment = members
        .iter()
        .map(|&at| stated[at].enacted.as_str())
        .min()
        .unwrap_or_default();
    // A title the dataset does not hold is a limit of the dataset. Saying that
    // nothing changed there would state a fact about the law nobody checked.
    if dataset.expressions(work)?.is_empty() {
        for &at in members {
            outcomes[at] = Some(residue(
                Stage::Window,
                format!("the dataset holds no expression of {work}"),
            ));
        }
        return Ok(Vec::new());
    }
    let windows = windows_after(dataset, work, earliest_enactment)?;
    // Where each amendment is in the order the laws state them.
    let stated_at: BTreeMap<&str, usize> = stated
        .iter()
        .enumerate()
        .map(|(at, amendment)| (amendment.address.amendment_id.as_str(), at))
        .collect();

    // Stage 2: the window. Every window the amendment's address changed in,
    // oldest first.
    let mut placed: BTreeMap<usize, Vec<Placed>> = BTreeMap::new();
    // The amendments a window after enactment was read for, and those whose
    // address that window's Code holds. One looked for and never found is not
    // quiet: nothing changed there because nothing is there.
    let mut looked_for: BTreeSet<usize> = BTreeSet::new();
    let mut found: BTreeSet<usize> = BTreeSet::new();
    // The amendments whose address is inside a provision a window added or
    // removed whole: the first such window, and the changes that hold the
    // address.
    let mut enclosed: BTreeMap<usize, Placed> = BTreeMap::new();
    for (window, view) in windows.iter().enumerate() {
        let later = SectionIndex::of(&view.later);
        let earlier = SectionIndex::of(&view.earlier);
        for &at in members {
            let amendment = &stated[at];
            if !window_can_hold(&amendment.enacted, &view.to.at) {
                continue;
            }
            looked_for.insert(at);
            let Some((section, under)) = address_path(&later, &earlier, &amendment.address) else {
                continue;
            };
            let renumbered_later = |by: &str| {
                stated_at.get(by).is_some_and(|&renumbering| {
                    renumbering > at && stated[renumbering].public_law == amendment.public_law
                })
            };
            let under = if amendment.amending_text.contains("as redesignated")
                || amendment.amending_text.contains("as so redesignated")
            {
                under
            } else {
                through_later_renumberings(under, &view.renumberings, renumbered_later)
            };
            let under =
                moved_by_another_law(view, &section, &under, &amendment.evidence).unwrap_or(under);
            found.insert(at);
            let candidates: Vec<usize> = (0..view.changes.len())
                .filter(|&change| is_at_or_below(&view.changes[change].path, &under))
                .collect();
            if !candidates.is_empty() {
                placed.entry(at).or_default().push(Placed {
                    window,
                    section,
                    address: under,
                    candidates,
                });
                continue;
            }
            // The diff lists a provision added or removed whole as one path,
            // and nothing beneath it. An address inside it changed all the
            // same (#336).
            let enclosing: Vec<usize> = (0..view.changes.len())
                .filter(|&change| {
                    let change = &view.changes[change];
                    change.whole.is_some() && is_at_or_below(&under, &change.path)
                })
                .collect();
            if enclosing.is_empty() {
                continue;
            }
            enclosed.entry(at).or_insert(Placed {
                window,
                section,
                address: under,
                candidates: enclosing,
            });
        }
    }
    // A new part that holds a new section is the change of the amendment that
    // adds the section, where no window changed anything under its address.
    // Any other amendment, such as one that amends a section a later
    // amendment moved out, did not make that change, so a person looks at it.
    for (at, inside) in std::mem::take(&mut enclosed) {
        if placed.contains_key(&at) {
            continue;
        }
        let added_whole = inside
            .candidates
            .iter()
            .all(|&change| windows[inside.window].changes[change].whole == Some(Whole::Added));
        if added_whole && only_inserts(&stated[at]) {
            placed.insert(at, vec![inside]);
        } else {
            enclosed.insert(at, inside);
        }
    }

    // Stage 3: resolve, in the first window that holds a change, with every
    // amendment addressed to the same section in the same window.
    let mut groups: BTreeMap<(usize, &str), Vec<usize>> = BTreeMap::new();
    for &at in members {
        match placed.get(&at).and_then(|found| found.first()) {
            Some(first) => groups
                .entry((first.window, first.section.as_str()))
                .or_default()
                .push(at),
            None if enclosed.contains_key(&at) => {
                let inside = &enclosed[&at];
                let view = &windows[inside.window];
                let holders: Vec<String> = inside
                    .candidates
                    .iter()
                    .map(|&change| {
                        let change = &view.changes[change];
                        let how = change.whole.map(|whole| whole.to_string());
                        format!("{} ({})", change.path, how.unwrap_or_default())
                    })
                    .collect();
                outcomes[at] = Some(Outcome::Residue(Residue {
                    stage: Stage::Window,
                    reason: format!(
                        "{} is inside {}, which the window changed whole. The method links \
                         such a provision only to an amendment that only adds, and only when \
                         the provision was added",
                        address_text(&stated[at].address),
                        holders.join(", ")
                    ),
                    quiet: false,
                    from: Some(view.from.clone()),
                    to: Some(view.to.clone()),
                    changes: inside
                        .candidates
                        .iter()
                        .map(|&change| view.changes[change].path.clone())
                        .collect(),
                }));
            }
            None if looked_for.contains(&at) && !found.contains(&at) => {
                let amendment = &stated[at];
                outcomes[at] = Some(residue(
                    Stage::Window,
                    format!(
                        "the Code the dataset holds has no {} in any window after {}",
                        address_text(&amendment.address),
                        amendment.enacted
                    ),
                ));
            }
            None => {
                let amendment = &stated[at];
                outcomes[at] = Some(Outcome::Residue(Residue {
                    stage: Stage::Window,
                    reason: format!(
                        "nothing under {} changed in any window after {}",
                        amendment.address.section.as_deref().unwrap_or_default(),
                        amendment.enacted
                    ),
                    quiet: true,
                    from: None,
                    to: None,
                    changes: Vec::new(),
                }));
            }
        }
    }
    for ((window, _), group) in groups {
        let view = &windows[window];
        let contenders: Vec<Contender> = group
            .iter()
            .map(|at| Contender {
                amendment_id: &stated[*at].address.amendment_id,
                public_law: &stated[*at].public_law,
                only_renumbers: stated[*at].operation == AmendingAction::Redesignate,
                units_named: units_its_words_name(&stated[*at]),
                only_strikes: only_strikes(&stated[*at]),
                only_inserts: only_inserts(&stated[*at]),
                rewrites: stated[*at].amending_text.contains("to read as follows"),
                evidence: &stated[*at].evidence,
                address: &placed[at][0].address,
                candidates: placed[at][0].candidates.clone(),
            })
            .collect();
        for (at, resolution) in group.iter().zip(resolve(&view.changes, &contenders)) {
            let changes_under_address = || {
                placed[at][0]
                    .candidates
                    .iter()
                    .map(|change| view.changes[*change].path.clone())
                    .collect()
            };
            outcomes[*at] = Some(match resolution {
                Resolution::Caused(caused) => Outcome::Linked(Linked {
                    from: view.from.clone(),
                    to: view.to.clone(),
                    under_address: changes_under_address(),
                    changes: one_for_each_path(
                        caused
                            .iter()
                            .map(|(change, found)| CausedChange {
                                path: view.changes[*change].path.clone(),
                                why: found.to_string(),
                            })
                            .collect(),
                    ),
                    later_windows: placed[at][1..]
                        .iter()
                        .map(|later| {
                            let view = &windows[later.window];
                            LaterWindow {
                                from: view.from.clone(),
                                to: view.to.clone(),
                                changes: later
                                    .candidates
                                    .iter()
                                    .map(|change| view.changes[*change].path.clone())
                                    .collect(),
                            }
                        })
                        .collect(),
                    olrc: olrc_classification(
                        classifications,
                        &placed[at][0].section,
                        &stated[*at].public_law,
                    ),
                }),
                Resolution::Stopped(reason) => Outcome::Residue(Residue {
                    stage: Stage::Resolve,
                    reason,
                    quiet: false,
                    from: Some(view.from.clone()),
                    to: Some(view.to.clone()),
                    changes: changes_under_address(),
                }),
            });
        }
    }
    Ok(windows
        .into_iter()
        .map(|view| (view.from, view.to))
        .collect())
}

/// The units an amendment's words name, read only when it quotes nothing
/// ([`units_named_in`] says why).
///
/// An amendment that quotes words takes at most one change by elimination
/// already, so only one that quotes nothing needs narrowing.
fn units_its_words_name(amendment: &Stated) -> Vec<Vec<Step>> {
    if amendment.evidence.quotes_anything() {
        return Vec::new();
    }
    units_named_in(&amendment.amending_text)
}

/// Whether an amendment only adds and inserts: the markup states `add` or
/// `insert` and nothing else beside the umbrella `amend`.
fn only_inserts(amendment: &Stated) -> bool {
    let actions = specific_actions(amendment);
    !actions.is_empty()
        && actions
            .iter()
            .all(|action| matches!(action, AmendingAction::Add | AmendingAction::Insert))
}

/// The actions the markup states for an amendment, without the umbrella
/// `amend` most instructions carry beside them.
fn specific_actions(amendment: &Stated) -> Vec<AmendingAction> {
    amendment
        .actions
        .iter()
        .copied()
        .filter(|action| *action != AmendingAction::Amend)
        .collect()
}

/// Whether an amendment only strikes and repeals.
///
/// The markup states `delete` or `repeal` and nothing else beside the umbrella
/// `amend`, and the bill quotes no words it inserts or enacts. *"Repeal and
/// reserve"* is not a strike alone: it leaves a `[Reserved]` in its place.
fn only_strikes(amendment: &Stated) -> bool {
    let actions = specific_actions(amendment);
    !actions.is_empty()
        && actions
            .iter()
            .all(|action| matches!(action, AmendingAction::Delete | AmendingAction::Repeal))
        && amendment.evidence.only_strikes()
}

/// The caused changes with each path once, in the order first met.
///
/// A diff reports a renumbered provision whose words also changed twice at one
/// path: as the move, and as the change to its words. One path is one link, so
/// the two become one entry, and its reason says both.
fn one_for_each_path(caused: Vec<CausedChange>) -> Vec<CausedChange> {
    let mut merged: Vec<CausedChange> = Vec::new();
    for change in caused {
        match merged.iter_mut().find(|kept| kept.path == change.path) {
            Some(kept) if kept.why != change.why => {
                kept.why = format!("{} Also: {}", kept.why, change.why);
            }
            Some(_) => {}
            None => merged.push(change),
        }
    }
    merged
}

/// Residue that stopped before any window was found.
fn residue(stage: Stage, reason: String) -> Outcome {
    Outcome::Residue(Residue {
        stage,
        reason,
        quiet: false,
        from: None,
        to: None,
        changes: Vec::new(),
    })
}

/// Whether a window that ends on `window_end` can hold a change made by a law
/// enacted on `enacted`. Both are ISO dates.
///
/// A window that ends on or before the enactment date holds the Code as it
/// read before the law existed, so the law cannot have changed anything in it.
pub fn window_can_hold(enacted: &str, window_end: &str) -> bool {
    window_end > enacted
}

/// Every window of a work that can hold a change made on `enacted`, oldest
/// first, each read and diffed once.
fn windows_after<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    work: &WorkId,
    enacted: &str,
) -> Result<Vec<WindowView>, DatasetError> {
    let held = dataset.expressions(work)?;
    let mut views = Vec::new();
    for pair in held.windows(2) {
        let (from, to) = (pair[0].id.clone(), pair[1].id.clone());
        if !window_can_hold(enacted, &to.at) {
            continue;
        }
        let (Some(earlier), Some(later)) =
            (dataset.get_expression(&from)?, dataset.get_expression(&to)?)
        else {
            continue;
        };
        let diff = dataset.compute_diff(&from, &to)?;
        let renumberings = renumberings(&dataset.storage().links_for_pair(&from, &to)?);
        let mut changes = Vec::new();
        collect_changes(&diff, &later.root, &renumberings, &mut changes);
        views.push(WindowView {
            from,
            to,
            earlier: earlier.root,
            later: later.root,
            changes,
            renumberings,
        });
    }
    Ok(views)
}

/// The paths an address names — its section's, and the provision's below
/// it — in the later document of a window or, when the later one does not
/// hold them, in the earlier one.
fn address_path(
    later: &SectionIndex,
    earlier: &SectionIndex,
    address: &AmendmentAddress,
) -> Option<(String, String)> {
    let section = address.section.as_deref()?;
    [later, earlier].into_iter().find_map(|index| {
        let [section] = index.get(section) else {
            return None;
        };
        let under = walk_down(section, &address.container).ok()?;
        Some((section.data.path.to_string(), under.data.path.to_string()))
    })
}

/// The path an address names, read through a renumbering that its own law
/// makes after it in the same window (#434).
///
/// A law acts in the order it states its amendments. Where it amends a
/// provision and a later amendment of it renumbers that provision, the Code
/// after the window prints the provision at its new number. 26 U.S.C. 67:
/// § 70110(a) of Pub. L. 119-21 amends 67(g), and § 70110(b)(2) then
/// redesignates (g) as (h) and inserts a new (g). The change § 70110(a) made
/// is at (h).
///
/// The address stays as it is where no renumbering, or more than one, moved
/// it to a new path, and where the renumbering is another law's or comes
/// earlier in the law: then the amendment names the provision by the number
/// it already has.
fn through_later_renumberings(
    under: String,
    renumberings: &Renumberings,
    renumbered_later: impl Fn(&str) -> bool,
) -> String {
    let moved_to: BTreeSet<String> = renumberings
        .iter()
        .filter(|((from, _), by)| {
            is_at_or_below(&under, from) && by.iter().any(|id| renumbered_later(id))
        })
        .map(|((from, to), _)| format!("{to}{}", &under[from.len()..]))
        .collect();
    match moved_to.into_iter().collect::<Vec<String>>().as_slice() {
        [one] => one.clone(),
        _ => under,
    }
}

/// The provision another law moved the address to in the window, as the
/// amendment's quoted words show it, or `None` (#391).
///
/// Two laws can change one section in one window, and the dataset can hold
/// no record of the other law. 26 U.S.C. 25C: another law redesignated (h) as
/// (i) and inserted a new (h). § 70505(a) of Pub. L. 119-21 amends "Section
/// 25C(h)" by striking words and inserting "placed in service after December
/// 31, 2025", and the Code after the window prints those words at (i). By its
/// number alone, the address is the new (h), which is the other law's.
///
/// The quoted words decide. The address moves only when all of these are so:
///
/// - the provision at the address before the window holds every string the
///   amendment strikes or quotes to mark a place, and it quotes one at least;
/// - no change under the address shows words the amendment adds;
/// - one provision, and only one, that the window added whole beside the
///   address (the same provision above it, the same level) shows every string
///   the amendment inserts and every block it enacts.
///
/// The OLRC's notes, which say which law moved a provision, are the next
/// evidence, but the dataset does not hold them (#380). The effective date
/// alone does not decide.
fn moved_by_another_law(
    view: &WindowView,
    section: &str,
    under: &str,
    evidence: &QuotedWords,
) -> Option<String> {
    let (above, own) = under.rsplit_once('/')?;
    let (level, _) = own.split_once('_')?;
    if under == section || evidence.additions() == 0 {
        return None;
    }
    let held_its_words = view
        .earlier
        .find_all(under)
        .into_iter()
        .any(|node| evidence.acts_on_words_in(&words_of_provision(node)));
    if !held_its_words {
        return None;
    }
    let adds = |change: &Change| {
        evidence
            .shown(&change.before, &change.after, change.added.as_ref(), &[])
            .into_iter()
            .filter(|shown| shown.adds)
            .count()
    };
    let shown_under_address = view
        .changes
        .iter()
        .any(|change| is_at_or_below(&change.path, under) && adds(change) > 0);
    if shown_under_address {
        return None;
    }
    let beside: Vec<&Change> = view
        .changes
        .iter()
        .filter(|change| change.whole == Some(Whole::Added))
        .filter(|change| {
            change
                .path
                .rsplit_once('/')
                .is_some_and(|(parent, segment)| {
                    parent == above && segment.starts_with(&format!("{level}_"))
                })
        })
        .filter(|change| adds(change) == evidence.additions())
        .collect();
    match beside.as_slice() {
        [moved_to] => Some(moved_to.path.clone()),
        _ => None,
    }
}

/// The words of a provision and of every provision below it, as one text.
fn words_of_provision(node: &DocumentNode) -> String {
    provision_words(node)
        .into_iter()
        .map(|field| field.text)
        .collect::<Vec<String>>()
        .join(" ")
}

/// Who made each renumbering in a window, by the two paths it moved a provision
/// between, as the window's `legislature.redesignated_as` links say.
type Renumberings = BTreeMap<(String, String), Vec<String>>;

fn renumberings(links: &[Link]) -> Renumberings {
    let path_of = |target: &Target| match target {
        Target::Change { path, .. } | Target::Node(path) => Some(path.clone()),
        _ => None,
    };
    let mut made_by = Renumberings::new();
    for link in links {
        if link.kind != LinkKind::new(LinkKind::REDESIGNATED_AS) {
            continue;
        }
        let amendment = link
            .payload
            .as_ref()
            .and_then(|payload| payload.value.get("amendment_id"))
            .and_then(|id| id.as_str());
        if let (Some(from), Some(to), Some(amendment)) =
            (path_of(&link.subject), path_of(&link.object), amendment)
        {
            made_by
                .entry((from, to))
                .or_default()
                .push(amendment.to_string());
        }
    }
    made_by
}

/// Every change in a diff, with its words before and after: a provision whose
/// own words changed, or one that was added, removed or renumbered.
///
/// `later` is the later document of the window. An added provision holds
/// every provision below it (#336), and the diff lists only the provision, so
/// its words after the change are read from `later`, with the provisions below
/// it (#432).
fn collect_changes(
    diff: &TreeDiff,
    later: &DocumentNode,
    renumberings: &Renumberings,
    changes: &mut Vec<Change>,
) {
    if !diff.changes.is_empty() {
        let joined = |value: fn(&FieldChangeEvent) -> &str| {
            diff.changes
                .iter()
                .map(value)
                .collect::<Vec<&str>>()
                .join(" ")
        };
        changes.push(Change::new(
            diff.root_path.clone(),
            joined(|field| &field.old_value),
            joined(|field| &field.new_value),
            None,
        ));
    }
    changes.extend(
        diff.added
            .iter()
            .map(|node| Change::added(node.path.to_string(), added_words(later, node))),
    );
    changes.extend(diff.removed.iter().map(|node| {
        Change::new(
            node.path.to_string(),
            own_text(node),
            String::new(),
            Some(Whole::Removed),
        )
    }));
    changes.extend(diff.moved.iter().map(|moved| {
        let paths = (moved.from.path.to_string(), moved.to.path.to_string());
        Change {
            renumbered_by: renumberings.get(&paths).cloned().unwrap_or_default(),
            ..Change::new(
                moved.to.path.to_string(),
                own_text(&moved.from),
                own_text(&moved.to),
                None,
            )
        }
    }));
    for child in &diff.child_diffs {
        collect_changes(child, later, renumberings, changes);
    }
}

/// The words of an added provision and of every provision below it, field by
/// field, as the window's later document prints them.
///
/// 26 U.S.C. 904(b)(5) prints subparagraphs (A) to (C) between its chapeau
/// and its continuation, and the block that enacted it holds them there too.
/// Its own words alone are not one run of the block (#432).
fn added_words(later: &DocumentNode, added: &NodeData) -> AddedWords {
    let own = own_text(added);
    let found = later
        .find_all(&added.path)
        .into_iter()
        .find(|found| found.data == *added);
    let Some(node) = found else {
        return AddedWords {
            fields: vec![own.clone()],
            own,
            below: Vec::new(),
        };
    };
    let fields = provision_words(node);
    AddedWords {
        below: fields
            .iter()
            .filter(|field| field.path != *added.path)
            .map(|field| field.text.clone())
            .collect(),
        fields: fields.into_iter().map(|field| field.text).collect(),
        own,
    }
}

/// One provision's own words, its five text fields joined by a space.
fn own_text(node: &NodeData) -> String {
    [
        node.heading.as_deref(),
        node.chapeau.as_deref(),
        node.content.as_deref(),
        node.proviso.as_deref(),
        node.continuation.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<&str>>()
    .join(" ")
}

fn is_at_or_below(path: &str, under: &str) -> bool {
    path == under
        || path
            .strip_prefix(under)
            .is_some_and(|rest| rest.starts_with('/'))
}
