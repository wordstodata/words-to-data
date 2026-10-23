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

pub mod default_code;
pub mod resolve;
pub mod usc;

use serde_json::json;

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
/// writing's node, such as `judicial/case_122262/opinion_9434365`, so a reader
/// can follow the link to the text that made the citation, a backend can index
/// it, and `LinkReader::links_for_path` answers "what does this case cite".
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
    /// The path of the writing this dataset holds the opinion as, when it
    /// holds it.
    held_as: Option<String>,
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

    /// An opinion this dataset holds, as the writing at the path `held_as`.
    ///
    /// Use `courtlistener::writing_path` to name it, so the link and the stored
    /// node cannot disagree about where the writing is.
    pub fn held(
        held_as: impl Into<String>,
        id: impl Into<String>,
        display: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            held_as: Some(held_as.into()),
        }
    }

    /// How an opinion outside the dataset is named: `judicial.opinion:11103682`.
    pub fn reference(&self) -> String {
        format!("judicial.opinion:{}", self.id)
    }

    fn target(&self) -> Target {
        match &self.held_as {
            Some(path) => Target::Node(path.clone()),
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
///
/// Version 2 (#309) reads a space before a subdivision's bracket, reads a dash
/// of any kind between two numbers, and resolves each end of a range.
///
/// Version 3 (#341) reads the OCR form `V.S.C.` as `U.S.C.` and names the
/// repair in the evidence, reads `U.S.C` with no last point, and reads a
/// citation with no section marker when a subdivision is written onto the
/// number (`7 U.S.C. 2015(o)(6)(G)`).
fn citation_rule() -> Method {
    Method::new("reporters-db laws.json U.S.C. patterns", 3)
}

/// The rule that reads a bare section reference under the default code an
/// opinion names (`default_code`, #318), at the version it is at now. A rule of
/// its own, because it reads text the U.S.C. patterns never read, and a reviewer
/// can then judge its links apart.
fn default_code_rule() -> Method {
    Method::new("bare section under the opinion's default code", 1)
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
        method: Some(match citation.default_code {
            Some(_) => default_code_rule(),
            None => citation_rule(),
        }),
        verification: VerificationState::MachineSuggested,
        // The matched text, so a reviewer can read what the rule read. A rule
        // has no reasoning beyond the text that satisfied it, and the repair it
        // made to read that text, when it made one.
        evidence: Some(Evidence {
            reasoning: Some(evidence_of(citation)),
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
                // The year the opinion says it reads the Code as in effect
                // for, when its default names one (ADR 0014, decisions 7 and
                // 8). Null for every other citation.
                "time_basis": citation.default_code.as_ref().and_then(|default| default.year.clone()),
            }),
        }),
        kind,
        object: Target::Node(path.to_string()),
        provenance,
    }
}

/// What a citation link carries as evidence: the words the rule read, and what
/// else the reading rests on.
///
/// A repair follows the words: `26 V.S.C. §§ 1361(a)(1) (OCR repair: "V.S.C."
/// read as "U.S.C.")`. A bare reference is read only because of the opinion's
/// default, so the default follows the words, with the writing that states it.
fn evidence_of(citation: &UscCitation) -> String {
    match (&citation.repair, &citation.default_code) {
        (Some(repair), _) => format!("{} ({repair})", citation.text),
        (None, Some(default)) => format!(
            "{} (title {} from the default stated in {}: \"{}\")",
            citation.text, default.title, default.stated_in, default.statement
        ),
        (None, None) => citation.text.clone(),
    }
}
