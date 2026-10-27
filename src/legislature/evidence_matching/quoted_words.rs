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
//! and each other mark on its own. Every quotation mark and every dash are
//! folded first, because the bill and the Code print them differently. The
//! bill quotes inside a quoted string with single marks, *"(determined by
//! substituting '1996' for '1997' …)"*, and the Code prints double ones, so a
//! single and a double mark are one mark (#433).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use super::code_edits::as_the_code_prints;
use crate::uslm::{AmendmentFacts, QuotedText};

/// The words one amendment states, ready to test against a change.
#[derive(Debug, Clone, Default)]
pub(super) struct QuotedWords {
    quoted: Vec<Quoted>,
    enacted: Vec<Block>,
    /// Every word the amendment states, for ranking a tie.
    words: BTreeSet<String>,
    /// A string it inserts or a block it enacts opens a new provision.
    opens_a_provision: bool,
}

/// One enacted block, as words without marks.
#[derive(Debug, Clone)]
struct Block {
    /// As the bill prints it.
    words: Vec<String>,
    /// As the OLRC can print it in the Code, where that differs
    /// ([`as_the_code_prints`]).
    as_the_code_prints: Vec<Vec<String>>,
}

impl Block {
    fn of(text: &str, enacted_on: &str) -> Self {
        Self {
            words: words_of(text),
            as_the_code_prints: as_the_code_prints(text, enacted_on)
                .iter()
                .map(|printed| words_of(printed))
                .collect(),
        }
    }

    /// How many words of the change the block shows, and whether it rewrote
    /// the provision, as [`Readings::shown_in`] says: the block as the bill
    /// prints it first, then as the OLRC can print it.
    fn shown_in(&self, readings: &Readings) -> Option<(usize, bool)> {
        std::iter::once(&self.words)
            .chain(&self.as_the_code_prints)
            .find_map(|words| readings.shown_in(words))
    }
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
    /// An enacted block (`<quotedContent>`), and not a quoted string.
    pub enacted: bool,
    /// An enacted block that holds every word the provision now prints: the
    /// amendment rewrote it.
    pub rewrote: bool,
    /// An enacted block that the change shows by a few words alone: the
    /// provision prints no more than [`FEW_WORDS`] words, and the block holds
    /// more. A heading such as "In general" is in many blocks, so it is weak
    /// evidence that this block made the change (#389).
    pub few_words: bool,
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

/// The most words a provision can print and still be shown by an enacted
/// block only weakly, when the block holds more words than that. 22 U.S.C.
/// 9613(g)(2)(E) prints "support—", and blocks of the same law that enact
/// other provisions hold "support" too (#389).
const FEW_WORDS: usize = 3;

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
    /// The evidence one instruction's stored facts give, for a law enacted on
    /// `enacted_on` (`2025-12-18`).
    pub(super) fn of(facts: &AmendmentFacts, enacted_on: &str) -> Self {
        let quoted: Vec<Quoted> = facts.quoted_text.iter().filter_map(Quoted::of).collect();
        let enacted: Vec<Block> = facts
            .enacted_text
            .iter()
            .map(|block| Block::of(&without_page_marks(block), enacted_on))
            .filter(|block| !block.words.is_empty())
            .collect();
        let words = quoted
            .iter()
            .flat_map(|quoted| quoted.tokens.iter())
            .chain(enacted.iter().flat_map(|block| &block.words))
            .filter(|token| is_word(token))
            .cloned()
            .collect();
        let opens_a_provision = quoted
            .iter()
            .filter(|quoted| quoted.direction == Direction::Inserted)
            .map(|quoted| quoted.tokens.clone())
            .chain(facts.enacted_text.iter().map(|block| tokens_of(block)))
            .any(|tokens| opens_with_a_designation(&tokens));
        Self {
            quoted,
            enacted,
            words,
            opens_a_provision,
        }
    }

    /// The evidence one item of an instruction that acts on many sections
    /// gives (#434): only the quoted strings and enacted blocks that the
    /// item's own words, `item_words`, state.
    ///
    /// The bill stores the words of the whole instruction, and the words of
    /// one item can show a change in another item's section. A string is the
    /// item's when its words hold it inside quotation marks. A block is the
    /// item's when its words hold the block's first words.
    pub(super) fn of_item(facts: &AmendmentFacts, enacted_on: &str, item_words: &str) -> Self {
        /// Enough words to tell one block from another.
        const FIRST_WORDS: usize = 12;
        let item_tokens = tokens_of(&without_page_marks(item_words));
        let item_words = words_of(&without_page_marks(item_words));
        let mut own = facts.clone();
        own.quoted_text.retain(|quoted| {
            contains_run(&item_tokens, &tokens_of(&format!("\"{}\"", quoted.text)))
        });
        own.enacted_text.retain(|block| {
            let words = words_of(&without_page_marks(block));
            let first = &words[..words.len().min(FIRST_WORDS)];
            !first.is_empty() && contains_run(&item_words, first)
        });
        Self::of(&own, enacted_on)
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

    /// Whether the amendment inserts or enacts words that open a new
    /// provision: a designation with words after it, at the start of the words
    /// or after a quotation mark. *Striking "Subparagraphs" and inserting "(I)
    /// In general.—Subparagraphs"* moves the words of a clause down into a new
    /// subclause (I).
    pub(super) fn opens_a_provision(&self) -> bool {
        self.opens_a_provision
    }

    /// Whether every word the bill quotes for this amendment is words it
    /// strikes: it enacts no block and inserts no string.
    pub(super) fn only_strikes(&self) -> bool {
        self.enacted.is_empty()
            && self
                .quoted
                .iter()
                .all(|quoted| quoted.direction == Direction::Struck)
    }

    /// The amendment's quoted strings and enacted blocks this change shows.
    ///
    /// `after` is the change's words after it, as one text. For a provision
    /// added whole, `added` gives the same words field by field, and a block
    /// is tested against them as [`AddedWords`] says.
    ///
    /// `later` are the amendments the same law makes after this one, to the
    /// same section. A later amendment can insert words inside words this
    /// one inserted, so the Code never prints this one's words as the bill
    /// quotes them. A quoted string is also shown when it shows with the
    /// words the later amendments inserted in this change taken out of it.
    pub(super) fn shown(
        &self,
        before: &str,
        after: &str,
        added: Option<&AddedWords>,
        later: &[&QuotedWords],
    ) -> Vec<Shown> {
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
                enacted: false,
                rewrote: false,
                few_words: false,
            });
        let undone = |text: &str| -> Vec<String> {
            without(&tokens_of(text), &later_insertions)
                .into_iter()
                .filter(|token| is_word(token))
                .collect()
        };
        let readings = match added {
            Some(added) => Readings {
                every_field: added.fields.iter().map(|field| words_of(field)).collect(),
                every_field_undone: added.fields.iter().map(|field| undone(field)).collect(),
                own: words_of(&added.own),
                own_undone: undone(&added.own),
                below: added.below.iter().map(|field| words_of(field)).collect(),
            },
            None => Readings {
                every_field: vec![words_of(after)],
                every_field_undone: vec![undone(after)],
                own: words_of(after),
                own_undone: undone(after),
                below: Vec::new(),
            },
        };
        let blocks = self.enacted.iter().enumerate().filter_map(|(at, block)| {
            let (count, rewrote) = block.shown_in(&readings)?;
            Some(Shown {
                said: format!("enacted text {} of {}", at + 1, self.enacted.len()),
                words: block.words.clone(),
                enacted: true,
                rewrote,
                few_words: count <= FEW_WORDS && block.words.len() > count,
            })
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

/// The words of a provision added whole, field by field (#432).
///
/// An added provision holds every provision below it (#336). A block shows it
/// when each field of it and of the provisions below is a run of the block's
/// words, in the order the Code prints them: the block holds the designations
/// of the provisions below between them.
///
/// The Code does not always print the words of a block as the bill does. The
/// publisher prints a marginal note, such as "Applicability.", inside the
/// words, and the Code's editors print "section 1101(a)(15) of title 8" for
/// "section 101(a)(15) of the Immigration and Nationality Act". So a block
/// shows the provision too when its own words are one run of the block, as
/// before. Where they are [`FEW_WORDS`] words or fewer, the block must also
/// hold, after them, the words of a field below of more words than that. A
/// few words alone, such as "In this subsection:" at 10 U.S.C. 9062(j)(4),
/// are in blocks that enact other provisions.
#[derive(Debug, Clone)]
pub(super) struct AddedWords {
    /// Each text field of the provision and of every provision below it, in
    /// the order the Code prints them: a continuation after the provisions
    /// it closes.
    pub fields: Vec<String>,
    /// The provision's own words, as one text.
    pub own: String,
    /// Each text field of the provisions below it, in the same order.
    pub below: Vec<String>,
}

/// The words of a change after it, ready to test against each block, each as
/// the Code prints them and with the words later amendments inserted taken
/// out.
struct Readings {
    /// The words of each field, in order.
    every_field: Vec<Vec<String>>,
    every_field_undone: Vec<Vec<String>>,
    /// The provision's own words.
    own: Vec<String>,
    own_undone: Vec<String>,
    /// The words of each field of the provisions below it, in order. Empty
    /// for every change but a provision added whole.
    below: Vec<Vec<String>>,
}

impl Readings {
    /// How many words of the change a block shows, and whether it shows them
    /// as the Code prints them: then the block holds every word the provision
    /// prints, and the amendment rewrote it. `None` when it shows none.
    ///
    /// The fields in order first, then the own words alone, as
    /// [`AddedWords`] says.
    fn shown_in(&self, block: &[String]) -> Option<(usize, bool)> {
        if runs_in_order(block, &self.every_field) {
            return Some((self.every_field.iter().map(Vec::len).sum(), true));
        }
        let own_counts = self.own_counts_in(block);
        if own_counts && contains_run(block, &self.own) {
            return Some((self.own.len(), true));
        }
        if runs_in_order(block, &self.every_field_undone) {
            return Some((self.every_field_undone.iter().map(Vec::len).sum(), false));
        }
        if own_counts && contains_run(block, &self.own_undone) {
            return Some((self.own_undone.len(), false));
        }
        None
    }

    /// Whether the provision's own words alone are evidence in this block:
    /// nothing was added below it, or its own words are more than a few, or
    /// the block holds them and, after them, a field below of more than a few
    /// words.
    fn own_counts_in(&self, block: &[String]) -> bool {
        self.below.is_empty()
            || self.own.len() > FEW_WORDS
            || self.below.iter().any(|field| {
                field.len() > FEW_WORDS && runs_in_order(block, &[self.own.clone(), field.clone()])
            })
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

/// Whether tokens open a new provision: a designation, such as `(I)`, with
/// words after it, at the start or after a quotation mark. The bill opens each
/// provision it quotes with a quotation mark: `thereof—"(1) 'calendar year
/// 2017'`. A designation inside a sentence, as in "subsection (e)(8) or", is a
/// reference and not a new provision.
fn opens_with_a_designation(tokens: &[String]) -> bool {
    (0..tokens.len())
        .filter(|&at| at == 0 || tokens[at - 1] == "\"")
        .any(|at| {
            matches!(
                &tokens[at..],
                [open, designation, close, next, ..]
                    if open == "(" && close == ")" && is_word(designation) && next != "("
            )
        })
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
        '\u{2018}' | '\u{2019}' | '`' | '\'' | '\u{201C}' | '\u{201D}' => '"',
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

/// Whether `after` holds a word more times than `before` does: the change
/// brought new words, and so it is not a strike alone.
pub(super) fn brings_new_words(before: &str, after: &str) -> bool {
    let mut left: BTreeMap<String, usize> = BTreeMap::new();
    for word in words_of(before) {
        *left.entry(word).or_default() += 1;
    }
    words_of(after)
        .into_iter()
        .any(|word| match left.get_mut(&word) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        })
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

/// Whether each of `runs` is a run of `tokens`, each after the one before it,
/// and at least one of them holds words. Words can stand between two runs.
fn runs_in_order(tokens: &[String], runs: &[Vec<String>]) -> bool {
    let mut from = 0;
    for run in runs.iter().filter(|run| !run.is_empty()) {
        match tokens[from..]
            .windows(run.len())
            .position(|at| at == run.as_slice())
        {
            Some(start) => from += start + run.len(),
            None => return false,
        }
    }
    runs.iter().any(|run| !run.is_empty())
}

fn contains_run(tokens: &[String], run: &[String]) -> bool {
    occurrences(tokens, run) > 0
}
