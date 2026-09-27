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
//! The words are compared as tokens: runs of letters and digits, lower case,
//! and each other mark on its own. Curly quotation marks and every dash are
//! folded first, because the bill and the Code print them differently.

use std::sync::LazyLock;

use regex::Regex;

use crate::uslm::{AmendmentFacts, QuotedText};

/// The words one amendment states, ready to test against a change.
#[derive(Debug, Clone, Default)]
pub(super) struct Evidence {
    quoted: Vec<Quoted>,
    /// Each enacted block, as words without marks.
    enacted: Vec<Vec<String>>,
    /// Every word the amendment states, for ranking a tie.
    words: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct Quoted {
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

impl Evidence {
    /// The evidence one instruction's stored facts give.
    pub(super) fn of(facts: &AmendmentFacts) -> Self {
        let quoted: Vec<Quoted> = facts
            .quoted_text
            .iter()
            .filter_map(Quoted::of)
            .collect();
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

    /// How many of the amendment's quoted strings and enacted blocks this
    /// change shows. Zero is no evidence.
    pub(super) fn strength(&self, before: &str, after: &str) -> usize {
        let (before_tokens, after_tokens) = (tokens_of(before), tokens_of(after));
        let shown_strings = self
            .quoted
            .iter()
            .filter(|quoted| quoted.is_shown(&before_tokens, &after_tokens))
            .count();
        let after_words = words_of(after);
        let shown_blocks = self
            .enacted
            .iter()
            .filter(|block| !after_words.is_empty() && contains_run(block, &after_words))
            .count();
        shown_strings + shown_blocks
    }

    /// The share of the words that differ between `before` and `after` that
    /// this amendment states, from 0 to 1.
    ///
    /// Only for ranking what the quoted words left tied: overlap is a hint,
    /// and never a reason on its own (`docs/adr/0013`).
    pub(super) fn overlap(&self, before: &str, after: &str) -> f64 {
        let before: std::collections::BTreeSet<String> = words_of(before).into_iter().collect();
        let after: std::collections::BTreeSet<String> = words_of(after).into_iter().collect();
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
        Some(Self { tokens, direction })
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

fn contains_run(tokens: &[String], run: &[String]) -> bool {
    occurrences(tokens, run) > 0
}
