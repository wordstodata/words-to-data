//! Stage 3: the changes under one section in one window, assigned to the
//! amendments addressed there, all together.
//!
//! Together, so that one amendment does not take another's change. A change
//! has one cause, except a provision edited in place: the diff reports one
//! paragraph as one change, and several amendments can edit it. 26 U.S.C.
//! 6041(a) carries five. The order of the steps is the order of trust:
//!
//! 0. **A renumbering the dataset already explains.** The bill's reading
//!    records each renumbering as a `legislature.redesignated_as` link that
//!    names its amendment. A renumbering quotes no words, so this is the only
//!    evidence it has, and it is the bill's own.
//! 1. **The quoted words.** A provision edited in place goes to every
//!    amendment that shows words of its own in it: words no other amendment
//!    there shows, alone or inside longer words. Any other change goes to the
//!    amendment whose quoted strings and enacted blocks it shows most. Where
//!    two show equally, word overlap ranks them; where that ties too, the
//!    change is held back and nobody gets it. An added or removed provision
//!    shows every string it holds, so it never has more than one cause.
//! 2. **The provision it rewrote.** A change inside a provision an amendment
//!    was given in step 1, and under that amendment's address, goes to it: a
//!    paragraph struck and enacted anew carries its subparagraphs with it. A
//!    provision several amendments edited does not say which of them made a
//!    change inside it, so it gives nothing this way.
//! 3. **Elimination.** What is left goes to the one amendment whose address
//!    covers it. An amendment that quotes words its changes do not show only
//!    takes a change this way when a single change is left to it, because its
//!    own words speak against every other.

use std::collections::{BTreeMap, BTreeSet};

use super::quoted_words::{QuotedWords, Shown};

/// One change in a window: a path, and its words before and after.
#[derive(Debug, Clone)]
pub(super) struct Change {
    pub path: String,
    pub before: String,
    pub after: String,
    /// For a renumbering, the amendments the dataset's
    /// `legislature.redesignated_as` links say made it. Empty for every other
    /// change.
    pub renumbered_by: Vec<String>,
}

impl Change {
    /// Whether the provision has words both before and after the change: it
    /// was edited, and not added, removed or emptied.
    fn is_edited_in_place(&self) -> bool {
        !self.before.trim().is_empty() && !self.after.trim().is_empty()
    }
}

/// One amendment addressed to the section: what it states, and the changes
/// under its own address, by their place in the window's list.
pub(super) struct Contender<'a> {
    pub amendment_id: &'a str,
    pub evidence: &'a QuotedWords,
    pub candidates: Vec<usize>,
}

/// What resolving gave one amendment.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Resolution {
    /// The changes it caused, in document order, each with how it was found.
    Caused(Vec<(usize, Found)>),
    /// Why it was given none.
    Stopped(String),
}

/// How a change was given to its amendment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Found {
    /// The change is a renumbering, and the dataset's redesignation link says
    /// this amendment made it.
    Renumbered,
    /// The change shows these of the amendment's quoted strings and enacted
    /// blocks.
    Quoted(Vec<String>),
    /// The change sits inside the provision at this path, which the
    /// amendment's quoted words placed.
    Inside(String),
    /// No other amendment's address, still wanting a change, covers it.
    OnlyAddress,
}

impl std::fmt::Display for Found {
    /// How the change was given to its amendment, as a sentence for the
    /// link's evidence.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Renumbered => write!(
                f,
                "the change is a renumbering, and the dataset's legislature.redesignated_as \
                 link says this amendment made it."
            ),
            Self::Quoted(shown) => write!(
                f,
                "the change shows the words the bill quotes: {}.",
                shown.join(", ")
            ),
            Self::Inside(path) => write!(
                f,
                "the change sits inside {path}, which the words the bill quotes placed."
            ),
            Self::OnlyAddress => write!(
                f,
                "the change is under the address, and no other amendment addressed there \
                 takes it."
            ),
        }
    }
}

/// Assign every change under one section to the amendments addressed there.
///
/// One answer for each contender, in the order given.
pub(super) fn resolve(changes: &[Change], contenders: &[Contender]) -> Vec<Resolution> {
    // Each change given so far, and the amendments that made it.
    let mut cause: BTreeMap<usize, Vec<(usize, Found)>> = BTreeMap::new();
    let mut held_back: BTreeSet<usize> = BTreeSet::new();
    let every_change: BTreeSet<usize> = contenders
        .iter()
        .flat_map(|contender| contender.candidates.iter().copied())
        .collect();

    // 0. A renumbering, which the dataset already says the cause of.
    for &at in &every_change {
        let named: Vec<usize> = contenders
            .iter()
            .enumerate()
            .filter(|(_, contender)| {
                contender.candidates.contains(&at)
                    && changes[at]
                        .renumbered_by
                        .iter()
                        .any(|id| id == contender.amendment_id)
            })
            .map(|(who, _)| who)
            .collect();
        if let [who] = named.as_slice() {
            cause.insert(at, vec![(*who, Found::Renumbered)]);
        }
    }

    // 1. The quoted words.
    for &at in &every_change {
        if cause.contains_key(&at) {
            continue;
        }
        let change = &changes[at];
        let claims: Vec<(usize, Vec<Shown>)> = contenders
            .iter()
            .enumerate()
            .filter(|(_, contender)| contender.candidates.contains(&at))
            .map(|(who, contender)| (who, contender.evidence.shown(&change.before, &change.after)))
            .filter(|(_, shown)| !shown.is_empty())
            .collect();
        // A provision edited in place can carry the edits of several
        // amendments, and every amendment that shows words of its own in it
        // made part of it. Words another amendment also states, alone or
        // inside longer words of its own, are not its own.
        //
        // A provision added or removed whole shows every string it holds, so
        // there a string of one's own says little, and one amendment is the
        // cause.
        let own: Vec<(usize, Found)> = claims
            .iter()
            .filter(|(who, shown)| {
                shown.iter().any(|words| {
                    claims.iter().all(|(other, theirs)| {
                        other == who || !theirs.iter().any(|them| words.is_within(them))
                    })
                })
            })
            .map(|(who, shown)| (*who, found_by(shown)))
            .collect();
        if change.is_edited_in_place() && !own.is_empty() {
            cause.insert(at, own);
            continue;
        }
        // The rest quote the same words, and the one that shows the most of
        // them, or overlaps the change most, is the cause.
        let Some(strongest) = claims.iter().map(|(_, shown)| shown.len()).max() else {
            continue;
        };
        let tied: Vec<usize> = claims
            .iter()
            .filter(|(_, shown)| shown.len() == strongest)
            .map(|(who, _)| *who)
            .collect();
        match rank_by_overlap(change, contenders, &tied) {
            Some(who) => {
                let found = claims
                    .iter()
                    .find(|(claimant, _)| *claimant == who)
                    .map(|(_, shown)| found_by(shown))
                    .unwrap_or(Found::Quoted(Vec::new()));
                cause.insert(at, vec![(who, found)]);
            }
            None => {
                held_back.insert(at);
            }
        }
    }

    // 2. The provision it rewrote. A diff can report two changes at one path,
    // so each provision and its amendment are kept once.
    let placed: BTreeSet<(String, usize)> = cause
        .iter()
        .flat_map(|(&at, causes)| {
            causes
                .iter()
                .map(move |(who, _)| (changes[at].path.clone(), *who))
        })
        .collect();
    for &at in &every_change {
        if cause.contains_key(&at) || held_back.contains(&at) {
            continue;
        }
        let around: Vec<&(String, usize)> = placed
            .iter()
            .filter(|(path, who)| {
                is_below(&changes[at].path, path) && contenders[*who].candidates.contains(&at)
            })
            .collect();
        let Some(deepest) = around.iter().map(|(path, _)| path.len()).max() else {
            continue;
        };
        // A provision several amendments edited does not say which of them
        // made a change inside it.
        if let [(path, who)] = around
            .iter()
            .filter(|(path, _)| path.len() == deepest)
            .copied()
            .collect::<Vec<_>>()
            .as_slice()
        {
            cause.insert(at, vec![(*who, Found::Inside(path.clone()))]);
        }
    }

    // 3. Elimination.
    let has_a_cause: BTreeSet<usize> = cause.values().flatten().map(|(who, _)| *who).collect();
    let mut wanted: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (who, contender) in contenders.iter().enumerate() {
        if has_a_cause.contains(&who) {
            continue;
        }
        let free: Vec<usize> = contender
            .candidates
            .iter()
            .copied()
            .filter(|at| !cause.contains_key(at) && !held_back.contains(at))
            .collect();
        let may_take = !contender.evidence.quotes_anything() || free.len() == 1;
        if may_take {
            for at in free {
                wanted.entry(at).or_default().push(who);
            }
        }
    }
    for (at, wanting) in wanted {
        match rank_by_overlap(&changes[at], contenders, &wanting) {
            Some(who) => {
                cause.insert(at, vec![(who, Found::OnlyAddress)]);
            }
            None => {
                held_back.insert(at);
            }
        }
    }

    contenders
        .iter()
        .enumerate()
        .map(|(who, contender)| {
            let caused: Vec<(usize, Found)> = cause
                .iter()
                .flat_map(|(&at, causes)| {
                    causes
                        .iter()
                        .filter(|(by, _)| *by == who)
                        .map(move |(_, found)| (at, found.clone()))
                })
                .collect();
            if !caused.is_empty() {
                return Resolution::Caused(caused);
            }
            Resolution::Stopped(why_stopped(changes, contender, &held_back))
        })
        .collect()
}

/// A change given by the words it shows, said as a reviewer reads them.
fn found_by(shown: &[Shown]) -> Found {
    Found::Quoted(shown.iter().map(|words| words.said.clone()).collect())
}

/// The one contender of `tied` whose words overlap the change most, or
/// `None` when no single one does.
fn rank_by_overlap(change: &Change, contenders: &[Contender], tied: &[usize]) -> Option<usize> {
    if let [only] = tied {
        return Some(*only);
    }
    let scored: Vec<(usize, f64)> = tied
        .iter()
        .map(|&who| {
            (
                who,
                contenders[who]
                    .evidence
                    .overlap(&change.before, &change.after),
            )
        })
        .collect();
    let best = scored.iter().map(|(_, score)| *score).fold(0.0, f64::max);
    let leaders: Vec<usize> = scored
        .iter()
        .filter(|(_, score)| *score == best)
        .map(|(who, _)| *who)
        .collect();
    match leaders.as_slice() {
        [one] if best > 0.0 => Some(*one),
        _ => None,
    }
}

fn why_stopped(changes: &[Change], contender: &Contender, held_back: &BTreeSet<usize>) -> String {
    let count = contender.candidates.len();
    if contender.candidates.iter().any(|at| held_back.contains(at)) {
        return "another amendment's words fit a change under its address as well as its own, and \
                nothing breaks the tie"
            .to_string();
    }
    let shows_its_words = contender.candidates.iter().any(|&at| {
        contender
            .evidence
            .strength(&changes[at].before, &changes[at].after)
            > 0
    });
    if contender.evidence.quotes_anything() && !shows_its_words {
        return format!(
            "the words it quotes show in none of the {count} change(s) under its address"
        );
    }
    format!("each of the {count} change(s) under its address is another amendment's")
}

fn is_below(path: &str, above: &str) -> bool {
    path.strip_prefix(above)
        .is_some_and(|rest| rest.starts_with('/'))
}
