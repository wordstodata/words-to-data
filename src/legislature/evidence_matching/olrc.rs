//! What the Office of Law Revision Counsel's classification says about an
//! amendment's section: the address when the markup gives none, and evidence
//! for a reviewer when it does.
//!
//! The `olrc.classified_from` links (#247) say which section of the Code each
//! section of a public law was classified to, and the kind of change. They
//! resolve to a section and no lower, so they cannot choose a change. They can
//! say that the publisher and the Code's editors agree on the section, and a
//! link's evidence says so when they do.
//!
//! **A note is not a change to the section's text.** A row whose description
//! is a note form — `nt`, `nts`, `nt [tbl]`, `nt new` — classifies a note
//! under the section, and the dataset holds no notes. `prec`, the heading
//! before a section, is the same. Such a row sits on the section's path and
//! says nothing of the section's own words, so it never corroborates a link
//! and never gives an address.
//!
//! **Only an amendment the markup does not address depends on these links**
//! ([`olrc_address`], #259). The markup wins: an amendment it addresses gets
//! the same answer with or without the links, and only its evidence says less.

use serde::Serialize;

use crate::link::{Link, LinkKind, Target};
use crate::olrc::law_section::{LawSection, classifications_of};
use crate::uslm::amendment_address::uslm_section_id;

/// What the classification says about one section, for one public law.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OlrcClassification {
    /// Sections of the law classified to a change to the section's text, as
    /// the table names them: `10101(b)(1)`.
    Text(Vec<String>),
    /// Sections of the law classified to the section only as a note, or as the
    /// heading before it.
    OnlyNotes(Vec<String>),
    /// No row of the law names the section.
    Nothing,
}

/// What the `olrc.classified_from` links among `links` say about the section at
/// `section_path`, for the public law numbered `public_law` (`119-21`).
pub fn olrc_classification(
    links: &[Link],
    section_path: &str,
    public_law: &str,
) -> OlrcClassification {
    let prefix = format!("olrc.classification:{public_law}:");
    let mut text = Vec::new();
    let mut notes = Vec::new();
    for link in links {
        if link.kind != LinkKind::new(LinkKind::CLASSIFIED_FROM) {
            continue;
        }
        if !matches!(&link.subject, Target::Node(path) if path.as_str() == section_path) {
            continue;
        }
        let Target::External { reference, .. } = &link.object else {
            continue;
        };
        let Some(law_section) = reference.strip_prefix(&prefix) else {
            continue;
        };
        let into = if descriptions(link)
            .iter()
            .all(|description| is_a_note(description))
        {
            &mut notes
        } else {
            &mut text
        };
        if !into.iter().any(|kept: &String| kept == law_section) {
            into.push(law_section.to_string());
        }
    }
    // A store hands its links back in its own order, so the sections are put in
    // the law's order for a reader that reads the same on every store.
    text.sort();
    notes.sort();
    match (text.is_empty(), notes.is_empty()) {
        (false, _) => OlrcClassification::Text(text),
        (true, false) => OlrcClassification::OnlyNotes(notes),
        (true, true) => OlrcClassification::Nothing,
    }
}

impl OlrcClassification {
    /// One sentence for a link's evidence.
    pub(super) fn sentence(&self, public_law: &str) -> String {
        let listed = |sections: &[String]| {
            sections
                .iter()
                .map(|section| format!("§ {section}"))
                .collect::<Vec<String>>()
                .join(", ")
        };
        match self {
            Self::Text(sections) => format!(
                "OLRC: the classification table places Pub. L. {public_law} {} at this section.",
                listed(sections)
            ),
            Self::OnlyNotes(sections) => format!(
                "OLRC: the classification table places Pub. L. {public_law} {} at this section \
                 only as a note, which says nothing of the section's own text.",
                listed(sections)
            ),
            Self::Nothing => format!(
                "OLRC: the dataset holds no classification of Pub. L. {public_law} at this section."
            ),
        }
    }
}

/// The section the OLRC's classification gives an amendment whose markup gives
/// none, and the rows of the table that give it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OlrcAddress {
    /// The section, as a USLM identifier: `/us/usc/t26/s530A`.
    pub section: String,
    /// The sections of the law the rows name, as the table writes them:
    /// `70204(a)(1)`.
    pub law_sections: Vec<String>,
}

/// The section the OLRC's classification gives the amendment at
/// `amendment_path` in the public law numbered `public_law` (`119-21`), when
/// it gives exactly one.
///
/// A row names the amendment's place in the law as `residue` reads it
/// ([`classifications_of`]), ranges included. A row whose descriptions are all
/// note forms, or `prec`, is left out: it never addresses a section's own
/// text ([`is_a_note`]). When the rows left name more than one section, the
/// table does not say which one this amendment acts on, and nothing is given.
pub(super) fn olrc_address(
    links: &[Link],
    public_law: &str,
    amendment_path: &str,
) -> Option<OlrcAddress> {
    let place = LawSection::of_path(amendment_path)?;
    let mut sections: Vec<String> = Vec::new();
    let mut law_sections: Vec<String> = Vec::new();
    for row in classifications_of(links, public_law, &place) {
        if row
            .descriptions
            .iter()
            .all(|description| is_a_note(description))
        {
            continue;
        }
        let section = uslm_section_of_path(&row.code_section)?;
        if !sections.contains(&section) {
            sections.push(section);
        }
        if !law_sections.contains(&row.law_section) {
            law_sections.push(row.law_section);
        }
    }
    match <[String; 1]>::try_from(sections) {
        Ok([section]) => Some(OlrcAddress {
            section,
            law_sections,
        }),
        Err(_) => None,
    }
}

/// The USLM identifier of the section at a structural path:
/// `uscode/title_26/…/section_530A` is `/us/usc/t26/s530A`.
///
/// A section can sit at more than one path over the release points a dataset
/// holds, and a classification link is stated for each. The identifier is the
/// same for all of them.
fn uslm_section_of_path(path: &str) -> Option<String> {
    let mut segments = path.split('/');
    let title = segments.nth(1)?.strip_prefix("title_")?;
    let section = path.rsplit('/').next()?.strip_prefix("section_")?;
    Some(uslm_section_id(title, section))
}

/// The descriptions a classification link carries, as the table wrote them.
fn descriptions(link: &Link) -> Vec<String> {
    link.payload
        .as_ref()
        .and_then(|payload| payload.value.get("descriptions"))
        .and_then(|descriptions| descriptions.as_array())
        .map(|descriptions| {
            descriptions
                .iter()
                .filter_map(|description| description.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a description classifies a note, or the heading before a section,
/// rather than the section's own text.
pub(crate) fn is_a_note(description: &str) -> bool {
    let first = description.split_whitespace().next().unwrap_or_default();
    matches!(first, "nt" | "nts" | "prec")
}
