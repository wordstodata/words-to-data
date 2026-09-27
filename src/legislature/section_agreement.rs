//! Whether an amendment's own words name the section its link points into.
//!
//! An amendment that names § 263A and whose link points inside § 263 is suspect
//! on its face, and nothing said so before (#239). Both halves of the question
//! are already in the dataset: the link names the amendment, whose bill says
//! where it acts, and the section is in the link's path. No model call, and no
//! diff.
//!
//! **Three outcomes, not two.** [`Outcome::CouldNotBeRead`] is its own case. An
//! amendment the resolver could not address is not an amendment that
//! disagrees, and reporting it as one would manufacture a false fault out of a
//! reader's limitation (#140).
//!
//! **The section comes from the bill's own markup.** The link says which
//! amendment caused the change, and the address resolver
//! ([`crate::uslm::amendment_address`]) reads where that amendment acts out of
//! the bill the dataset holds: the amending line's citation, the publisher's
//! `<ref>` beside a section of an Act, and a new section's own `SEC.` heading.
//! Before #248 this check read the amendment's words off the link with a prose
//! reader of its own, which had to guess at quotations, cross-references and
//! declined citations. The resolver reads the markup, where the quoted text is
//! already set apart, so none of those guesses remain. A link whose amendment
//! the resolver cannot address is the third outcome, with the resolver's
//! reason.
//!
//! **It does not decide.** A disagreement is a reason for a person to look, and
//! nothing here is stored (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! An amendment may lawfully name one section and act on a provision in another,
//! because the drafter said so, so the output is a queue and not a fault list.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::dataset::{DatasetError, bill_document};
use crate::link::{Link, LinkKind, Target, Window, amendment_reference_parts};
use crate::query::{Answer, LinkQuery};
use crate::storage::Storage;
use crate::uslm::amendment_address::{AmendmentAddress, addresses_in};

/// What the check found about one link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The named section and the path's section are the same.
    Agrees,
    /// They differ. A reviewer should look.
    Disagrees,
    /// The amendment's naming could not be established, so the two sides were
    /// never compared. Never a disagreement.
    CouldNotBeRead,
}

/// One link, and what the check found about it.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The link, by the short id `settle` accepts.
    ///
    /// The id leads a row because this list is a review queue and the id is the
    /// one field a reviewer copies out of it (#227).
    pub id: String,
    /// The structural path the link points into.
    pub path: String,
    /// The window the link was recorded over.
    ///
    /// Two links for one amendment over two windows are otherwise two rows a
    /// reader cannot tell apart (#184).
    pub window: Option<Window>,
    /// The section the amendment acts on, as its bill's markup addresses it,
    /// when it could be addressed.
    pub named_section: Option<String>,
    /// The section the path sits in.
    pub path_section: Option<String>,
    pub outcome: Outcome,
    /// Why the amendment could not be addressed, in the resolver's own words.
    ///
    /// Only ever set on [`Outcome::CouldNotBeRead`]. A third case with no reason
    /// reads as a silent gap, and the whole point of the case is that the limit
    /// is named (#140).
    pub reason: Option<String>,
}

/// How one window's links came out.
///
/// Per window, because the split between windows is the signal that filed this:
/// one dataset showed 4% of its first window's links disagreeing and 29% of its
/// second window's, and a figure folded over the whole dataset hides that (#239).
#[derive(Debug, Clone, Serialize)]
pub struct WindowTally {
    pub window: Window,
    /// Every amendment link recorded over this window.
    pub checked: usize,
    pub agrees: usize,
    pub disagrees: usize,
    pub could_not_be_read: usize,
    /// The share of this window's links a reviewer is asked to look at.
    ///
    /// A field rather than something a reader works out, so the denominator is
    /// stated rather than guessed: it is **every** link in the window, including
    /// the ones the check could not read. Those are links it did not clear, and
    /// a share taken over the read ones alone would rise as the reader got
    /// worse. The counts are beside it, so the other reading is there for
    /// whoever wants it.
    ///
    /// Zero for a window with no links, rather than a division by zero.
    pub disagreeing_share: f64,
}

/// What the check found over a dataset.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// One tally per window that carries an amendment link, oldest first.
    pub windows: Vec<WindowTally>,
    /// The queue: every link that asks something of a reviewer, suspect first.
    ///
    /// A link that agrees asks nothing, so it is counted in its window's tally
    /// and not listed. The disagreements lead, and the links whose naming could
    /// not be read follow them: those are a limit of the reader rather than
    /// a suspect match, and they want a different kind of attention.
    ///
    /// No finer ordering inside either group. Ordering the disagreements by how
    /// well each is corroborated is what a reviewer would want next, and these
    /// links carry no corroboration figure to order them by — `score-amendments`
    /// is what attaches one.
    pub rows: Vec<Row>,
}

/// Check every amendment link a query names.
///
/// The query says **which** links to check — a bill, a window, a path, or any
/// combination of them ([`LinkQuery`]) — and the kind is fixed to
/// `legislature.amended_by`, because the question is about an amendment's own
/// words and no other kind has any.
///
/// **The query's limit is ignored.** A per-window share must be taken over every
/// link in the window, and a fetch that stopped at twenty rows would report the
/// share of a screenful as the share of a window. A caller showing a screenful
/// bounds what it prints, as `redesignation-report` does.
pub fn section_agreement<S: Storage>(
    dataset: &S,
    query: &LinkQuery,
) -> Result<Report, DatasetError> {
    let mut amendments = query.clone();
    amendments.kind = Some(LinkKind::AMENDED_BY.to_string());
    amendments.limit = None;
    let Answer { rows: links, .. } = dataset.links_matching(&amendments)?;

    let mut addresses = Addresses::default();
    let mut checked: Vec<Row> = Vec::with_capacity(links.len());
    for link in &links {
        checked.push(check(link, &mut addresses, dataset)?);
    }
    let windows = tally_by_window(&checked);

    // The queue. Every link that agrees is already counted in its window's
    // tally, and listing it would put a row a reviewer must read past in front
    // of the rows they were looking for.
    let mut rows: Vec<Row> = checked
        .into_iter()
        .filter(|row| row.outcome != Outcome::Agrees)
        .collect();
    rows.sort_by(|one, other| {
        queue_place(one.outcome)
            .cmp(&queue_place(other.outcome))
            // Then by where in the Code it sits, so two runs over one dataset
            // give one order.
            .then_with(|| one.path.cmp(&other.path))
            .then_with(|| one.id.cmp(&other.id))
    });

    Ok(Report { windows, rows })
}

/// Where an outcome sits in the queue: the suspect links, then the unread ones.
fn queue_place(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Disagrees => 0,
        Outcome::CouldNotBeRead => 1,
        // Never queued. Kept exhaustive so that a fourth outcome cannot be
        // added without deciding where it belongs.
        Outcome::Agrees => 2,
    }
}

/// One tally for each window the rows name, oldest first.
///
/// A row whose subject names no window is left out rather than gathered under a
/// window of our choosing: a link with no window cannot answer which window's
/// share it belongs in.
fn tally_by_window(rows: &[Row]) -> Vec<WindowTally> {
    let mut by_window: BTreeMap<Window, WindowTally> = BTreeMap::new();
    for row in rows {
        let Some(window) = row.window.clone() else {
            continue;
        };
        let tally = by_window.entry(window.clone()).or_insert(WindowTally {
            window,
            checked: 0,
            agrees: 0,
            disagrees: 0,
            could_not_be_read: 0,
            disagreeing_share: 0.0,
        });
        tally.checked += 1;
        match row.outcome {
            Outcome::Agrees => tally.agrees += 1,
            Outcome::Disagrees => tally.disagrees += 1,
            Outcome::CouldNotBeRead => tally.could_not_be_read += 1,
        }
    }

    by_window
        .into_values()
        .map(|mut tally| {
            if tally.checked > 0 {
                tally.disagreeing_share = tally.disagrees as f64 / tally.checked as f64;
            }
            tally
        })
        .collect()
}

/// What the check finds about one link.
fn check<S: Storage>(
    link: &Link,
    addresses: &mut Addresses,
    dataset: &S,
) -> Result<Row, DatasetError> {
    let path = link.subject.path().unwrap_or_default().to_string();
    let path_section = section_in_path(&path);
    let (named_section, reason) = match addresses.of(&link.object, dataset)? {
        Ok(section) => (Some(section), None),
        Err(why) => (None, Some(why)),
    };

    let outcome = match (&named_section, &path_section) {
        (Some(named), Some(sitting)) if same_section(named, sitting) => Outcome::Agrees,
        (Some(_), Some(_)) => Outcome::Disagrees,
        _ => Outcome::CouldNotBeRead,
    };

    Ok(Row {
        id: crate::review::short_id(&link.id()).to_string(),
        path,
        window: link.subject.window(),
        named_section,
        path_section,
        outcome,
        reason,
    })
}

/// The addresses of every amendment of every bill the links name, read once
/// for each bill.
///
/// A bill's document is the largest thing this check opens, and a thousand
/// links name one bill, so each bill is read once and its addresses kept.
#[derive(Default)]
struct Addresses {
    /// By bill, then by amendment id. `None` for a bill the dataset holds no
    /// document for.
    by_bill: HashMap<String, Option<HashMap<String, AmendmentAddress>>>,
}

impl Addresses {
    /// The section number the amendment a link names acts on, or why it is
    /// not known.
    fn of<S: Storage>(
        &mut self,
        object: &Target,
        dataset: &S,
    ) -> Result<Result<String, String>, DatasetError> {
        let Target::External { reference, .. } = object else {
            return Ok(Err("the link names no amendment".to_string()));
        };
        let Some((bill_id, amendment_id)) = amendment_reference_parts(reference) else {
            return Ok(Err(format!("{reference} is not an amendment of a bill")));
        };
        if !self.by_bill.contains_key(bill_id) {
            let read = read_bill(dataset, bill_id)?;
            self.by_bill.insert(bill_id.to_string(), read);
        }
        let Some(amendments) = &self.by_bill[bill_id] else {
            return Ok(Err(format!(
                "the dataset holds no document for bill {bill_id}"
            )));
        };
        let Some(address) = amendments.get(amendment_id) else {
            return Ok(Err(format!(
                "bill {bill_id} states no amendment {amendment_id}"
            )));
        };
        Ok(match (&address.section, &address.unresolved) {
            (Some(section), _) => Ok(section_number(section).to_string()),
            (None, Some(reason)) => Err(reason.to_string()),
            (None, None) => Err("the amendment could not be addressed".to_string()),
        })
    }
}

/// Every amendment address of one bill, by amendment id, or `None` when the
/// dataset holds no document for it.
fn read_bill<S: Storage>(
    dataset: &S,
    bill_id: &str,
) -> Result<Option<HashMap<String, AmendmentAddress>>, DatasetError> {
    let Some(legislature) = dataset.legislature() else {
        return Ok(None);
    };
    let Some(document) = bill_document(dataset, legislature, bill_id)? else {
        return Ok(None);
    };
    Ok(Some(
        addresses_in(&document.root)
            .into_iter()
            .map(|address| (address.amendment_id.clone(), address))
            .collect(),
    ))
}

/// The section number out of a USLM identifier: `174A` from `/us/usc/t26/s174A`.
fn section_number(uslm_id: &str) -> &str {
    uslm_id
        .rsplit_once("/s")
        .map_or(uslm_id, |(_, section)| section)
}

/// Whether two spellings of a section number name one section.
///
/// Case-folded, because 10,085 of the Code's section numbers end in a letter and
/// a bill may write `45X` where a path segment carries `45x`.
///
/// Dash-folded on **both** sides. The Code prints `300gg–11` with an en dash and
/// prose writes `300gg-11` with a hyphen, and comparing the two character by
/// character answers that the Code has no such section (#141). Folding one side
/// only moves the mismatch, which is what
/// [`crate::citation::usc::fold_dashes`] says in its own note.
fn same_section(one: &str, other: &str) -> bool {
    let fold = |section: &str| crate::citation::usc::fold_dashes(section).to_ascii_lowercase();
    fold(one) == fold(other)
}

/// The section a structural path sits in: `263` in `…/section_263/subsection_a`.
///
/// `None` for a path above a section, such as a whole part of a title. Nothing
/// is compared there, because a path that sits in no section cannot disagree
/// with one.
fn section_in_path(path: &str) -> Option<String> {
    path.split('/')
        .find_map(|segment| segment.strip_prefix("section_"))
        .map(str::to_string)
}
