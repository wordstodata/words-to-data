//! How the words a bill quotes show which change an amendment made.
//!
//! A change has a before text and an after text. The bill states two kinds of
//! words, and each kind is tested against those texts:
//!
//! - **A quoted string** (`<quotedText>`). A struck string is in the before
//!   text more often than in the after text. An inserted string is in the
//!   after text more often than in the before text. A string with neither
//!   action before it must only appear a different number of times. A string
//!   that marks a place — the `"(3)"` in *inserting after "(3)"* — appears
//!   the same number of times on both sides, so it shows nothing, and that is
//!   the right answer for it.
//! - **An enacted block** (`<quotedContent>`). The after text of an added or
//!   rewritten provision is a run of the block's words.
//!
//! A law's amendments act in order, and a later one can insert words inside
//! the words an earlier one inserted. 26 U.S.C. 6041(a) prints "…receiving
//! such tips and a separate accounting of any amount of qualified overtime
//! compensation…)", where the earlier amendment quotes "…receiving such
//! tips)". So a quoted string is also tested with the later amendments'
//! inserted words taken out of the change, and so is an enacted block. Only
//! inserted words are taken out: words a later amendment struck are gone, and
//! nothing in the change says where they stood.
//!
//! # Too little to decide
//!
//! A change shows nothing for an amendment when every string it shows is made
//! of `COMMON_WORDS` alone — `"and"`, `", or"`, `"the"` — unless those words
//! are all the change struck or inserted. A struck "and" shows in every list
//! whose end moved, so under a wide address it points at many changes, and
//! the one it chose was wrong (42 U.S.C. 1396a(e)(14)(D)(iv), #259). Where a
//! paragraph lost an "and" and nothing else, the "and" is the change, and it
//! still decides.
//!
//! The words are compared as tokens: runs of letters and digits, lower case,
//! and each other mark on its own. Curly quotation marks and every dash are
//! folded first, because the bill and the Code print them differently.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::uslm::{AmendmentFacts, QuotedText};

/// The words one amendment states, ready to test against a change.
#[derive(Debug, Clone, Default)]
pub(super) struct QuotedWords {
    quoted: Vec<Quoted>,
    /// Each enacted block, as words without marks.
    enacted: Vec<Vec<String>>,
    /// Every word the amendment states, for ranking a tie.
    words: BTreeSet<String>,
}

/// One of an amendment's quoted strings or enacted blocks that a change
/// shows.
#[derive(Debug, Clone)]
pub(super) struct Shown {
    /// As a reviewer reads it: `struck "2023"`, `inserted "2031"`,
    /// `enacted text 1 of 2`.
    pub said: String,
    /// Its words, as they are compared.
    words: Vec<String>,
    /// An enacted block that holds every word the provision now prints: the
    /// amendment rewrote it.
    pub rewrote: bool,
}

impl Shown {
    /// Whether `other` states these words too: the same words, or more words
    /// around them. A struck "$600" is within a struck "of $600 or more".
    pub(super) fn is_within(&self, other: &Shown) -> bool {
        contains_run(&other.words, &self.words)
    }

    /// Whether every word here is one of the [`COMMON_WORDS`]: `"and"`,
    /// `"; or"`, `"the"`.
    fn is_common_words(&self) -> bool {
        self.words
            .iter()
            .filter(|token| is_word(token))
            .all(|word| COMMON_WORDS.contains(&word.as_str()))
    }
}

/// The words that join and qualify the Code's clauses. A quoted string made
/// of these alone is too little to say which change an amendment made: a
/// struck "and" shows in every list whose end moved.
const COMMON_WORDS: &[&str] = &[
    "a", "an", "and", "any", "as", "at", "be", "by", "each", "for", "he", "in", "is", "it", "may",
    "not", "of", "on", "or", "shall", "such", "than", "that", "the", "there", "this", "to",
    "which", "with",
];

#[derive(Debug, Clone)]
struct Quoted {
    /// The words as the bill quotes them, for a reviewer to read.
    text: String,
    tokens: Vec<String>,
    direction: Direction,
}

/// What a quoted string's action says the Code should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Fewer times after than before.
    Struck,
    /// More times after than before.
    Inserted,
    /// A different number of times.
    Either,
}

impl QuotedWords {
    /// The evidence one instruction's stored facts give.
    pub(super) fn of(facts: &AmendmentFacts) -> Self {
        let quoted: Vec<Quoted> = facts.quoted_text.iter().filter_map(Quoted::of).collect();
        let enacted: Vec<Vec<String>> = facts
            .enacted_text
            .iter()
            .map(|block| words_of(&without_page_marks(block)))
            .filter(|words| !words.is_empty())
            .collect();
        let words = quoted
            .iter()
            .flat_map(|quoted| quoted.tokens.iter())
            .chain(enacted.iter().flatten())
            .filter(|token| is_word(token))
            .cloned()
            .collect();
        Self {
            quoted,
            enacted,
            words,
        }
    }

    /// Whether the bill quotes any words for this amendment at all.
    ///
    /// An amendment that quotes words and whose words show in no change has
    /// evidence against every change, which is not the same as having none.
    pub(super) fn quotes_anything(&self) -> bool {
        !self.quoted.is_empty() || !self.enacted.is_empty()
    }

    /// Whether the bill quotes any short string (`<quotedText>`) for this
    /// amendment.
    pub(super) fn quotes_strings(&self) -> bool {
        !self.quoted.is_empty()
    }

    /// The amendment's quoted strings and enacted blocks this change shows.
    ///
    /// `later` are the amendments the same law makes after this one, to the
    /// same section. A later amendment can insert words inside words this
    /// one inserted, so the Code never prints this one's words as the bill
    /// quotes them. A quoted string is also shown when it shows with the
    /// words the later amendments inserted in this change taken out of it.
    pub(super) fn shown(&self, before: &str, after: &str, later: &[&QuotedWords]) -> Vec<Shown> {
        let (before_tokens, after_tokens) = (tokens_of(before), tokens_of(after));
        let later_insertions: Vec<&[String]> = later
            .iter()
            .flat_map(|amendment| amendment.inserted_in(&before_tokens, &after_tokens))
            .collect();
        let (before_undone, after_undone) = (
            without(&before_tokens, &later_insertions),
            without(&after_tokens, &later_insertions),
        );
        let strings = self
            .quoted
            .iter()
            .filter(|quoted| {
                quoted.is_shown(&before_tokens, &after_tokens)
                    || quoted.is_shown(&before_undone, &after_undone)
            })
            .map(|quoted| Shown {
                said: quoted.described(),
                words: quoted.tokens.clone(),
                rewrote: false,
            });
        let after_words = words_of(after);
        let after_words_undone: Vec<String> = after_undone
            .iter()
            .filter(|token| is_word(token))
            .cloned()
            .collect();
        let blocks = self
            .enacted
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                (!after_words.is_empty() && contains_run(block, &after_words))
                    || (!after_words_undone.is_empty() && contains_run(block, &after_words_undone))
            })
            .map(|(at, block)| Shown {
                said: format!("enacted text {} of {}", at + 1, self.enacted.len()),
                words: block.clone(),
                rewrote: !after_words.is_empty() && contains_run(block, &after_words),
            });
        let shown: Vec<Shown> = strings.chain(blocks).collect();
        // Common words alone decide nothing, unless they are all the change
        // struck or inserted.
        let stated: BTreeSet<&String> = shown.iter().flat_map(|words| &words.words).collect();
        let whole_change = differing_words(&before_tokens, &after_tokens)
            .iter()
            .all(|word| stated.contains(word));
        if shown.iter().all(Shown::is_common_words) && !whole_change {
            return Vec::new();
        }
        shown
    }

    /// The strings this amendment inserts that a change shows inserted.
    fn inserted_in<'a>(&'a self, before: &[String], after: &[String]) -> Vec<&'a [String]> {
        self.quoted
            .iter()
            .filter(|quoted| quoted.direction == Direction::Inserted)
            .filter(|quoted| quoted.is_shown(before, after))
            .map(|quoted| quoted.tokens.as_slice())
            .collect()
    }

    /// The share of the words that differ between `before` and `after` that
    /// this amendment states, from 0 to 1.
    ///
    /// Only for ranking what the quoted words left tied: overlap is a hint,
    /// and never a reason on its own (`docs/adr/0013`).
    pub(super) fn overlap(&self, before: &str, after: &str) -> f64 {
        let before: BTreeSet<String> = words_of(before).into_iter().collect();
        let after: BTreeSet<String> = words_of(after).into_iter().collect();
        let differing: Vec<&String> = before.symmetric_difference(&after).collect();
        if differing.is_empty() {
            return 0.0;
        }
        let stated = differing
            .iter()
            .filter(|word| self.words.contains(**word))
            .count();
        stated as f64 / differing.len() as f64
    }
}

impl Quoted {
    fn of(quoted: &QuotedText) -> Option<Self> {
        let tokens = tokens_of(&quoted.text);
        if tokens.is_empty() {
            return None;
        }
        let direction = match quoted.action.as_deref() {
            Some("delete" | "repeal") => Direction::Struck,
            Some("insert" | "add") => Direction::Inserted,
            _ => Direction::Either,
        };
        Some(Self {
            text: quoted.text.clone(),
            tokens,
            direction,
        })
    }

    fn described(&self) -> String {
        let verb = match self.direction {
            Direction::Struck => "struck",
            Direction::Inserted => "inserted",
            Direction::Either => "quoted",
        };
        format!("{verb} \"{}\"", self.text)
    }

    fn is_shown(&self, before: &[String], after: &[String]) -> bool {
        let (was, is) = (
            occurrences(before, &self.tokens),
            occurrences(after, &self.tokens),
        );
        match self.direction {
            Direction::Struck => was > is,
            Direction::Inserted => is > was,
            Direction::Either => was != is,
        }
    }
}

/// The page marks a public law prints inside the text it enacts: `139 STAT. 82`.
fn without_page_marks(text: &str) -> String {
    static PAGE_MARK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\d+\s+STAT\.\s+\d+").expect("the page mark must compile"));
    PAGE_MARK.replace_all(text, " ").into_owned()
}

/// Lower-case tokens: each run of letters and digits, and each other mark on
/// its own, after quotation marks and dashes are folded.
pub(super) fn tokens_of(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            word.extend(character.to_lowercase());
            continue;
        }
        if !word.is_empty() {
            tokens.push(std::mem::take(&mut word));
        }
        if !character.is_whitespace() {
            tokens.push(fold_mark(character).to_string());
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

/// The words of a text, without the marks between them.
fn words_of(text: &str) -> Vec<String> {
    tokens_of(text)
        .into_iter()
        .filter(|token| is_word(token))
        .collect()
}

fn is_word(token: &str) -> bool {
    token.chars().all(char::is_alphanumeric)
}

fn fold_mark(character: char) -> char {
    match character {
        '\u{2018}' | '\u{2019}' | '`' => '\'',
        '\u{201C}' | '\u{201D}' => '"',
        _ if crate::citation::usc::DASHES.contains(&character) => '-',
        other => other,
    }
}

/// How many times `run` appears in `tokens`.
fn occurrences(tokens: &[String], run: &[String]) -> usize {
    if run.is_empty() || run.len() > tokens.len() {
        return 0;
    }
    tokens.windows(run.len()).filter(|at| *at == run).count()
}

/// The words that appear a different number of times in `before` and
/// `after`: what a change struck or inserted.
fn differing_words(before: &[String], after: &[String]) -> BTreeSet<String> {
    let count = |tokens: &[String]| {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for word in tokens.iter().filter(|token| is_word(token)) {
            *counts.entry(word.clone()).or_default() += 1;
        }
        counts
    };
    let (was, is) = (count(before), count(after));
    was.keys()
        .chain(is.keys())
        .filter(|word| was.get(*word) != is.get(*word))
        .cloned()
        .collect()
}

/// `tokens` with every appearance of each of `runs` taken out.
fn without(tokens: &[String], runs: &[&[String]]) -> Vec<String> {
    let mut left: Vec<String> = tokens.to_vec();
    for run in runs.iter().filter(|run| !run.is_empty()) {
        let mut kept = Vec::with_capacity(left.len());
        let mut at = 0;
        while at < left.len() {
            if left[at..].starts_with(run) {
                at += run.len();
            } else {
                kept.push(left[at].clone());
                at += 1;
            }
        }
        left = kept;
    }
    left
}

fn contains_run(tokens: &[String], run: &[String]) -> bool {
    occurrences(tokens, run) > 0
}
