//! Stage 3: the changes under one section in one window, assigned to the
//! amendments addressed there, all together.
//!
//! Together, so that one change has one cause and one amendment does not take
//! another's change. The order of the steps is the order of trust:
//!
//! 0. **A renumbering the dataset already explains.** The bill's reading
//!    records each renumbering as a `legislature.redesignated_as` link that
//!    names its amendment. A renumbering quotes no words, so this is the only
//!    evidence it has, and it is the bill's own.
//! 1. **The quoted words.** A change goes to the amendment whose quoted strings
//!    and enacted blocks it shows most. Where two show equally, word overlap
//!    ranks them; where that ties too, the change is held back and nobody gets
//!    it.
//! 2. **The provision it rewrote.** A change inside a provision an amendment
//!    was given in step 1, and under that amendment's address, goes to it: a
//!    paragraph struck and enacted anew carries its subparagraphs with it.
//! 3. **Elimination.** What is left goes to the one amendment whose address
//!    covers it. An amendment that quotes words its changes do not show only
//!    takes a change this way when a single change is left to it, because its
//!    own words speak against every other.

use std::collections::{BTreeMap, BTreeSet};

use super::quoted_words::QuotedWords;

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
    let mut cause: BTreeMap<usize, (usize, Found)> = BTreeMap::new();
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
            cause.insert(at, (*who, Found::Renumbered));
        }
    }

    // 1. The quoted words.
    for &at in &every_change {
        if cause.contains_key(&at) {
            continue;
        }
        let change = &changes[at];
        let claims: Vec<(usize, Vec<String>)> = contenders
            .iter()
            .enumerate()
            .filter(|(_, contender)| contender.candidates.contains(&at))
            .map(|(who, contender)| (who, contender.evidence.shown(&change.before, &change.after)))
            .filter(|(_, shown)| !shown.is_empty())
            .collect();
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
                let shown = claims
                    .iter()
                    .find(|(claimant, _)| *claimant == who)
                    .map(|(_, shown)| shown.clone())
                    .unwrap_or_default();
                cause.insert(at, (who, Found::Quoted(shown)));
            }
            None => {
                held_back.insert(at);
            }
        }
    }

    // 2. The provision it rewrote.
    let placed: Vec<(String, usize)> = cause
        .iter()
        .map(|(&at, (who, _))| (changes[at].path.clone(), *who))
        .collect();
    for &at in &every_change {
        if cause.contains_key(&at) || held_back.contains(&at) {
            continue;
        }
        let inside = placed
            .iter()
            .filter(|(path, who)| {
                is_below(&changes[at].path, path) && contenders[*who].candidates.contains(&at)
            })
            .max_by_key(|(path, _)| path.len());
        if let Some((path, who)) = inside {
            cause.insert(at, (*who, Found::Inside(path.clone())));
        }
    }

    // 3. Elimination.
    let has_a_cause: BTreeSet<usize> = cause.values().map(|(who, _)| *who).collect();
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
        match wanting.as_slice() {
            [who] => {
                cause.insert(at, (*who, Found::OnlyAddress));
            }
            _ => {
                if let Some(who) = rank_by_overlap(&changes[at], contenders, &wanting) {
                    cause.insert(at, (who, Found::OnlyAddress));
                } else {
                    held_back.insert(at);
                }
            }
        }
    }

    contenders
        .iter()
        .enumerate()
        .map(|(who, contender)| {
            let caused: Vec<(usize, Found)> = cause
                .iter()
                .filter(|(_, (by, _))| *by == who)
                .map(|(&at, (_, found))| (at, found.clone()))
                .collect();
            if !caused.is_empty() {
                return Resolution::Caused(caused);
            }
            Resolution::Stopped(why_stopped(changes, contender, &held_back))
        })
        .collect()
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
