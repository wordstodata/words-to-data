//! The edits the OLRC makes to the words of a law when it prints them in the
//! Code (#433).
//!
//! The OLRC prints an enacted block in the Code, and it edits some of its
//! words as it prints them. A block that it edited is not one run of the
//! words the Code prints, so the block does not show the change it made. This
//! module gives the words of a block as the OLRC prints them, for the edits
//! that the bill's own words fix exactly:
//!
//! - **A reference to the Code.** *"section 552b of title 5, United States
//!   Code"* is printed *"section 552b of title 5"*.
//! - **A reference to a law, with its place in the Code.** *"section
//!   102(c)(4) of the America COMPETES Reauthorization Act of 2010 (42 U.S.C.
//!   6622(c)(4))"* is printed *"section 6622(c)(4) of title 42"*, and a later
//!   *"such section 102(c)(4)"* is printed *"such section 6622(c)(4) of title
//!   42"*. Where the provision is in the same title, the OLRC prints *"of this
//!   title"*. The title of the provision is not known here, so the words are
//!   given both ways.
//! - **The date of enactment.** *"the date of the enactment of this Act"* and
//!   *"the enactment of the DFC Modernization and Reauthorization Act of
//!   2025"* are printed as the date the law was enacted, *"December 18,
//!   2025"*.
//!
//! The OLRC does not make each edit each time: 46 U.S.C. 70103(c)(9) prints
//! *"the date of enactment of the Coast Guard Authorization Act of 2025"* as
//! the bill does. So these words are another reading of the block, and never
//! the only one. They
//! show a change only where every word of the change is in them, in the order
//! the Code prints it, as the bill's own words must.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// The texts of `block` as the OLRC can print it, for a law enacted on
/// `enacted_on` (`2025-12-18`). Each differs from `block`. Empty when none of
/// the known edits applies.
pub(super) fn as_the_code_prints(block: &str, enacted_on: &str) -> Vec<String> {
    let edited = without_united_states_code(block);
    let edited = with_the_date_of_enactment(&edited, enacted_on);
    let mut readings: Vec<String> = [Title::Named, Title::This]
        .into_iter()
        .map(|title| with_code_citations(&edited, title))
        .filter(|reading| reading != block)
        .collect();
    readings.dedup();
    readings
}

/// *"title 5, United States Code"* as *"title 5"*.
fn without_united_states_code(text: &str) -> String {
    static TITLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b(title \d+[A-Z]?),\s+United States Code\b")
            .expect("the title pattern must compile")
    });
    TITLE.replace_all(text, "$1").into_owned()
}

/// How the OLRC names the title of a section it cites.
#[derive(Clone, Copy)]
enum Title {
    /// *"of title 42"*.
    Named,
    /// *"of this title"*: the cited section is in the title the provision is in.
    This,
}

/// Each *"section S of the … Act (NN U.S.C. Z)"* as *"section Z of title
/// NN"*, and each *"such section S"* after it as *"such section Z of title
/// NN"*.
fn with_code_citations(text: &str, title: Title) -> String {
    static CITED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"\bsection (\w+(?:\(\w+\))*) of (?:the )?[^()]{1,160}? \((\d+) U\.\s?S\.\s?C\. (\w+(?:-\w+)?(?:\(\w+\))*)\)",
        )
        .expect("the citation pattern must compile")
    });
    let of_title = |number: &str| match title {
        Title::Named => format!("of title {number}"),
        Title::This => "of this title".to_string(),
    };
    let mut such_sections: Vec<(String, String)> = Vec::new();
    let mut edited = CITED
        .replace_all(text, |cited: &Captures| {
            let printed = format!("section {} {}", &cited[3], of_title(&cited[2]));
            such_sections.push((
                format!("such section {}", &cited[1]),
                format!("such {printed}"),
            ));
            printed
        })
        .into_owned();
    for (stated, printed) in such_sections {
        edited = edited.replace(&stated, &printed);
    }
    edited
}

/// *"the date of the enactment of this Act"*, and the same words for a unit of
/// the law or for a law named by its title, as the date the law was enacted:
/// *"December 18, 2025"*. `text` unchanged when `enacted_on` is not a date.
///
/// A law named by its title can be another law, enacted on another date. Then
/// these words do not show the change, and the words the bill prints decide,
/// as before.
fn with_the_date_of_enactment(text: &str, enacted_on: &str) -> String {
    static ENACTMENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"\b(?:the )?(?:date of )?(?:the )?enactment of (?:this (?:Act|title|subtitle|division|part|subpart|chapter|subchapter|section|subsection|paragraph|subparagraph|clause|subclause)\b|the [A-Z][^()]{0,120}?\bAct(?: of \d{4}| for Fiscal Year \d{4})?)",
        )
        .expect("the enactment pattern must compile")
    });
    let Ok(date) = crate::date::date_str_to_date(enacted_on) else {
        return text.to_string();
    };
    let printed = format!("{} {}, {}", date.month(), date.day(), date.year());
    ENACTMENT.replace_all(text, printed.as_str()).into_owned()
}
