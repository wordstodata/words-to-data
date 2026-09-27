//! Where an amendment sits in its public law, and the OLRC rows that name that
//! place.
//!
//! The OLRC classifies each section of a public law, and a row names it the way
//! the Statutes print it: `10101(b)(3)`. An amendment's node in the stored law
//! carries the same place as a structural path:
//! `publiclawdocument_119-21/title_I/subtitle_A/section_10101/subsection_b/paragraph_3`.
//! Both are read here into the same form, a section number and the
//! designations below it, so the two can be compared.

use serde::Serialize;

use crate::link::{Link, LinkKind, Target};

/// A place in a public law: the section number, then each designation below
/// it, outermost first. `10101(b)(3)` is `["10101", "b", "3"]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LawSection(Vec<String>);

impl LawSection {
    /// The place an amendment's node sits at, read from its structural path in
    /// the stored law. `None` when the path names no section of the law.
    pub(super) fn of_path(path: &str) -> Option<Self> {
        let trail = path
            .split('/')
            .skip_while(|segment| !segment.starts_with("section_"))
            .map(|segment| segment.split_once('_').map(|(_, number)| number.to_string()))
            .collect::<Option<Vec<String>>>()?;
        (!trail.is_empty()).then_some(Self(trail))
    }

    /// A section of the law as a row writes it, such as `10101(b)(3)`.
    fn of_row(written: &str) -> Self {
        let mut parts = written.split('(');
        let mut trail = vec![parts.next().unwrap_or_default().trim().to_string()];
        trail.extend(parts.map(|part| part.trim_end_matches(')').to_string()));
        Self(trail)
    }

    /// Whether one place holds the other. A row for `10101(b)` names an
    /// amendment at `10101(b)(3)`, and a row for `10104(a)` names part of an
    /// amendment that rewrites the whole of section 10104.
    fn overlaps(&self, other: &Self) -> bool {
        self.0.iter().zip(&other.0).all(|(one, two)| one == two)
    }
}

impl std::fmt::Display for LawSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (number, designations) = self.0.split_first().expect("a place has a section");
        write!(f, "{number}")?;
        for designation in designations {
            write!(f, "({designation})")?;
        }
        Ok(())
    }
}

/// One OLRC row that names an amendment's place in its law.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Classification {
    /// The section of the law, as the table writes it: `10101(b)(3)`.
    pub law_section: String,
    /// The section of the Code it was classified to, as a structural path.
    pub code_section: String,
    /// The kind of change, in the table's own words. Empty when the section
    /// is amended; `nt` for a note.
    pub descriptions: Vec<String>,
}

/// Every classification among `links` that names `place` in the public law
/// numbered `public_law` (`119-21`).
pub(super) fn classifications_of(
    links: &[Link],
    public_law: &str,
    place: &LawSection,
) -> Vec<Classification> {
    let prefix = format!("olrc.classification:{public_law}:");
    let mut found = Vec::new();
    for link in links {
        if link.kind != LinkKind::new(LinkKind::CLASSIFIED_FROM) {
            continue;
        }
        let (Target::Node(code_section), Target::External { reference, .. }) =
            (&link.subject, &link.object)
        else {
            continue;
        };
        let Some(law_section) = reference.strip_prefix(&prefix) else {
            continue;
        };
        if !LawSection::of_row(law_section).overlaps(place) {
            continue;
        }
        found.push(Classification {
            law_section: law_section.to_string(),
            code_section: code_section.clone(),
            descriptions: descriptions(link),
        });
    }
    // A store hands its links back in its own order, so the rows are put in one
    // order for a reader that reads the same on every store.
    found.sort_by(|one, other| {
        (&one.law_section, &one.code_section).cmp(&(&other.law_section, &other.code_section))
    });
    found
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
