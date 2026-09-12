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

use crate::link::{Evidence, Link, LinkKind, Provenance, Target, VerificationState};
use resolve::{CitedSection, Resolution};
use usc::UscCitation;

/// The opinion a citation was read out of.
///
/// An opinion is not a work in the core model yet, so it is named as something
/// outside the core, in the namespace of the link that mentions it. That is the
/// same shape `link::amendment_reference` uses for an amendment, which is also
/// not core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opinion {
    /// How this opinion is identified where it came from, such as a
    /// CourtListener opinion id.
    pub id: String,
    /// What to call it on a page: `Obergefell v. Hodges, 576 U.S. 644 (2015)`.
    pub display: String,
}

impl Opinion {
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
        }
    }

    /// How an opinion is named as a link target: `judicial.opinion:11103682`.
    pub fn reference(&self) -> String {
        format!("judicial.opinion:{}", self.id)
    }

    fn target(&self) -> Target {
        Target::External {
            reference: self.reference(),
            display: self.display.clone(),
        }
    }
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
        method: Some("reporters-db laws.json U.S.C. patterns".to_string()),
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
        object: Target::Provision(path.to_string()),
        provenance,
    }
}
