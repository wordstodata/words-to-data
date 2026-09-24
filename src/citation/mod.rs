//! Citations found in the text of an opinion, and the links they become.
//!
//! An opinion citing a statute is the link this project exists to record, and
//! nobody publishes it. CourtListener finds a statute citation with eyecite,
//! fails to resolve it, and throws it away: the resolver routes a
//! `FullLawCitation` to `NO_MATCH_RESOURCE`, the citation graph is opinion-to-
//! opinion and cannot hold a statute, and the only trace left is display markup
//! with no identifier, `<span class="citation no-link">21 U.S.C. § 846</span>`.
//! The evidence is in `docs/research/courtlistener-formats.md`, section 8.
//!
//! So the extractor is ours. [`usc`] reads the citations; [`resolve`] says what
//! this dataset can point at; [`cites_links`] turns the pair into links.
//!
//! # What a citation resolves to
//!
//! A structural path, and no more. A provision has no identity of its own in the
//! model yet (#93), so a link names where a provision sits rather than what it
//! is (`docs/adr/0001-structural-paths-locate-not-identify.md`). Every link made
//! here moves when identity arrives, which is why the citation text travels with
//! it as evidence.
//!
//! # What is in scope
//!
//! The U.S. Code, and nothing else. `laws.json` holds 371 keys; the rest arrive
//! with the corpus that needs them. `Pub. L.`, `Stat.` and `C.F.R.` are named in
//! #52 as out of scope, along with the two eyecite defects that belong to them.

pub mod resolve;
pub mod usc;

use serde_json::json;

use crate::dataset::WorkId;
use crate::link::{Evidence, Link, LinkKind, Provenance, Target, VerificationState};
use crate::method::Method;
use resolve::{CitedSection, Resolution};
use usc::UscCitation;

/// The opinion a citation was read out of.
///
/// It is named one of two ways, and which one is a statement about the dataset
/// rather than a style choice.
///
/// [`Opinion::held`] is for an opinion this dataset carries, since #53 put court
/// opinions in datasets. Then the subject is a [`Target::Node`] naming the
/// node, so a reader can follow the link to the text that made the citation, a
/// backend can index it, and `LinkReader::links_for_path` answers "what does this
/// case cite".
///
/// [`Opinion::new`] is for an opinion the dataset does **not** carry — a citation
/// read out of a text held somewhere else. Then the subject is
/// [`Target::External`], on the same shape `link::amendment_reference` uses for an
/// amendment, and a reader is told plainly that the citing document is not in the
/// file and the link's subject cannot be checked against it.
///
/// The word used to be the strain in this. The variant was `Target::Provision`,
/// and an opinion is not a provision. #147 renamed it to [`Target::Node`],
/// which is what the variant always meant: a node in this dataset, by path. See
/// `docs/research/a-court-opinion-in-the-core.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opinion {
    /// How this opinion is identified where it came from, such as a
    /// CourtListener opinion id.
    pub id: String,
    /// What to call it on a page: `Obergefell v. Hodges, 576 U.S. 644 (2015)`.
    pub display: String,
    /// The work this dataset holds the opinion as, when it holds it.
    held_as: Option<WorkId>,
}

impl Opinion {
    /// An opinion this dataset does not hold.
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            held_as: None,
        }
    }

    /// An opinion this dataset holds, as the work at `held_as`.
    ///
    /// Use `courtlistener::work_id` to name it, so the link and the stored node
    /// cannot disagree about where the opinion is.
    pub fn held(held_as: WorkId, id: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            held_as: Some(held_as),
        }
    }

    /// How an opinion outside the dataset is named: `judicial.opinion:11103682`.
    pub fn reference(&self) -> String {
        format!("judicial.opinion:{}", self.id)
    }

    fn target(&self) -> Target {
        match &self.held_as {
            Some(work) => Target::Node(work.to_string()),
            None => Target::External {
                reference: self.reference(),
                display: self.display.clone(),
            },
        }
    }
}

/// The rule that reads a U.S.C. citation, at the version it is at now.
///
/// Raise the version when the rule's answers change — when it starts reading a
/// citation it used to miss, or stops reading one it used to take. Editing a
/// comment or renaming a variable is not such a change
/// (`crate::method::Method`).
fn citation_rule() -> Method {
    Method::new("reporters-db laws.json U.S.C. patterns", 1)
}

/// A link for every provision a citation resolved to, saying the opinion cites
/// it.
///
/// A section that did not resolve makes no link. A link is a statement that can
/// be checked, and a path the dataset does not hold cannot be checked by the
/// party reading it; the [`Resolution`] still carries "out of scope" for a
/// caller to report, which is the answer a researcher needs and is never "not
/// found".
///
/// The state is always [`VerificationState::MachineSuggested`]: a rule matched
/// some text, and no person has looked at it.
pub fn cites_links(opinion: &Opinion, citation: &UscCitation, cited: &[CitedSection]) -> Vec<Link> {
    cited
        .iter()
        .flat_map(|section| {
            let paths = match &section.resolution {
                Resolution::Provision { paths } => paths.as_slice(),
                _ => &[],
            };
            paths
                .iter()
                .map(|path| cites_link(opinion, citation, section, path))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// One link: this opinion cites the provision at this path.
fn cites_link(
    opinion: &Opinion,
    citation: &UscCitation,
    section: &CitedSection,
    path: &str,
) -> Link {
    let kind = LinkKind::new(LinkKind::CITES);
    let provenance = Provenance {
        source: "rule:usc_citation".to_string(),
        method: Some(citation_rule()),
        verification: VerificationState::MachineSuggested,
        // The matched text, so a reviewer can read what the rule read. A rule
        // has no reasoning beyond the text that satisfied it.
        evidence: Some(Evidence {
            reasoning: Some(citation.text.clone()),
            ..Evidence::default()
        }),
        raw_score: None,
        // No clock reading. Nothing about this statement depends on when the
        // rule ran, and the same text gives the same answer on any day.
        timestamp: None,
        corroboration: None,
    };

    Link {
        subject: opinion.target(),
        // The namespace is read from the kind rather than written again, so the
        // two cannot drift apart.
        payload: Some(crate::link::KindPayload {
            namespace: kind.namespace().to_string(),
            value: json!({
                "title": citation.title,
                // As the opinion wrote it, brackets and all. The path stops at
                // the section, so this is the only record that the opinion named
                // a subsection.
                "section": section.section,
                "uslm_id": section.uslm_id,
                "start": citation.start,
            }),
        }),
        kind,
        object: Target::Node(path.to_string()),
        provenance,
    }
}
