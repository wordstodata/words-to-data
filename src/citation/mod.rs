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

pub mod carry;
pub mod default_code;
pub mod resolve;
pub mod usc;

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::json;

use crate::link::{Evidence, Link, LinkKind, Provenance, Target, VerificationState};
use crate::method::Method;
use resolve::CitedSection;
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
    /// The date the court decided the case, `YYYY-MM-DD`, when it is known:
    /// the cluster's `date_filed`, which is also the date of the case's
    /// expression (ADR 0014, decision 7; #317).
    decided: Option<String>,
}

impl Opinion {
    /// An opinion this dataset does not hold.
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            held_as: None,
            decided: None,
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
            decided: None,
        }
    }

    /// The same opinion, decided on `date` (`YYYY-MM-DD`). Each of its
    /// citation links carries the date, and is resolved in the printing held
    /// nearest it (#317).
    pub fn decided_on(self, date: impl Into<String>) -> Self {
        Self {
            decided: Some(date.into()),
            ..self
        }
    }

    /// The date the court decided the case, when it is known.
    pub fn decided(&self) -> Option<&str> {
        self.decided.as_deref()
    }

    /// The path of the node this dataset holds the opinion as, or `None` for
    /// an opinion it does not hold.
    pub fn held_as(&self) -> Option<&str> {
        self.held_as.as_deref()
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
///
/// Version 4 (#316) makes a link from the paragraph or footnote that holds the
/// citation to the subdivision the court wrote, and keeps every place in the
/// payload. Version 3 made a link from the writing to the section.
///
/// Version 5 (#366) reads a range of subdivisions after one section,
/// `26 U. S. C. §§994(a)(1)-(3)`, as a citation of each subdivision in the
/// range: a link to each, and each place holds the whole written form. Version
/// 4 read the first end alone and lost the rest.
///
/// Version 6 (#410) reads a designation of more than one character: a roman
/// numeral, `(ii)` or `(II)`, a doubled letter, `(cc)`, and a number and a
/// capital letter, `(14A)`. Version 5 stopped before it, so
/// `7 U.S.C. § 2015(o)(4)(A)(ii)` was a link to (A). In the same release
/// (#413), a footnote mark against the full stop after a section number,
/// `§5311.6`, is no part of the number when the writing has that footnote
/// ([`without_footnote_marks`]).
///
/// Version 7 (#449) reads the edition of the Code that a citation names in
/// brackets after it, `8 U.S.C. § 1105a(a) (1976 ed.)` or `26 U.S.C. § 162
/// (1988)`. Each place of the link records the edition, and the evidence ends
/// with it. The link and its object do not change.
fn citation_rule() -> Method {
    Method::new("reporters-db laws.json U.S.C. patterns", 7)
}

/// The rule that reads a bare section reference under the default code an
/// opinion names (`default_code`, #318), at the version it is at now. A rule of
/// its own, because it reads text the U.S.C. patterns never read, and a reviewer
/// can then judge its links apart.
///
/// Version 2 (#316): the link shape of [`citation_rule`] version 4.
///
/// Version 3 (#366): a range of subdivisions after one section, `section
/// 41(b)(1)-(3)`, is read as [`citation_rule`] version 5 reads it.
///
/// Version 4 (#410): a designation of more than one character,
/// `section 199A(c)(3)(A)(ii)`, is read as [`citation_rule`] version 6 reads it,
/// and so is a footnote mark after a full stop, `IRC section 179.17` (#413).
fn default_code_rule() -> Method {
    Method::new("bare section under the opinion's default code", 4)
}

/// A link for every provision a citation resolved to, saying the passage of
/// the opinion cites it.
///
/// The object is the deepest provision the written subdivision leads to
/// ([`resolve::Reached`]): `26 U.S.C. § 174(a)(1)` is a link to
/// `…/section_174/subsection_a/paragraph_1`, and `§ 174(e)`, in a printing with
/// no (e), is a link to the section that names `(e)` as the part that did not
/// resolve (ADR 0014, decision 6).
///
/// A section that did not resolve makes no link. A link is a statement that can
/// be checked, and a path the dataset does not hold cannot be checked by the
/// party reading it; the [`Resolution`](resolve::Resolution) still carries "out of scope" for a
/// caller to report, which is the answer a researcher needs and is never "not
/// found".
///
/// The state is always [`VerificationState::MachineSuggested`]: a rule matched
/// some text, and no person has looked at it.
///
/// One citation at a time. Give every citation of one passage to
/// [`cites_links_in`], so that a passage that cites one subdivision twice is
/// one link that holds both places.
pub fn cites_links(opinion: &Opinion, citation: &UscCitation, cited: &[CitedSection]) -> Vec<Link> {
    cites_links_in(opinion, &[(citation.clone(), cited.to_vec())])
}

/// The links of every citation one passage holds: one for each provision a
/// citation leads to, with every place the passage cites it.
///
/// A link is identified by its subject and object
/// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`), so
/// two citations of one subdivision in one paragraph state one link. Stored one
/// at a time, the second would replace the first, and the first place would be
/// lost. Here the places are gathered into one payload, in the order of the
/// text.
pub fn cites_links_in(
    opinion: &Opinion,
    citations: &[(UscCitation, Vec<CitedSection>)],
) -> Vec<Link> {
    let mut gathered: Vec<Gathered> = Vec::new();
    for (citation, cited) in citations {
        for section in cited {
            for reached in &section.reached {
                let place = Place {
                    start: citation.start,
                    written: section.section.clone(),
                    words: citation.text.clone(),
                    unresolved: reached.unresolved.clone(),
                    edition: citation.edition.clone(),
                };
                match gathered.iter_mut().find(|link| link.object == reached.path) {
                    Some(link) => link.add(citation, place),
                    None => gathered.push(Gathered::new(citation, section, &reached.path, place)),
                }
            }
        }
    }
    gathered
        .into_iter()
        .map(|link| link.into_link(opinion))
        .collect()
}

/// The citations of one writing, each given to the passage that holds its
/// first word.
#[derive(Debug, Clone, Default)]
pub struct PassageCitations<'a> {
    /// Each passage, by its path, with the citations that start in it, in the
    /// order of the passages. A citation's `start` is an offset in the words
    /// of its passage.
    pub by_passage: Vec<(&'a str, Vec<UscCitation>)>,
    /// The citation-shaped text the reader declined, with offsets in the
    /// passages joined as [`citations_by_passage`] reads them.
    pub report: usc::FindReport,
    /// How many of the citations are bare references read under `default`.
    pub bare: usize,
}

/// Read the citations of one writing from its passages, and give each to the
/// passage that holds its first word (ADR 0014, decision 5; #316).
///
/// The passages are read as one text, joined by a blank line, as the writing's
/// whole text was read before #316. A rule that finds paragraphs can find a
/// break inside a citation: *Torres* ends one paragraph with "I.R.C." and starts
/// the next with "§ 199A(b)(4)(B)". Read one passage at a time, that citation
/// is lost. Read as one text, it is a citation that starts in the first
/// passage and runs on into the next, and its words say so.
///
/// `default` is the default code the writing's case states, if any
/// ([`default_code::of_case`]): bare references are read under it.
pub fn citations_by_passage<'a>(
    passages: &[(&'a str, &'a str)],
    default: Option<&default_code::DefaultCode>,
) -> PassageCitations<'a> {
    const BREAK: &str = "\n\n";
    let mut starts = Vec::with_capacity(passages.len());
    let mut joined = String::new();
    for (index, (_, words)) in passages.iter().enumerate() {
        if index > 0 {
            joined.push_str(BREAK);
        }
        starts.push(joined.len());
        joined.push_str(words);
    }

    let footnotes: Vec<&str> = passages
        .iter()
        .filter_map(|(path, _)| path.rsplit_once("/footnote_"))
        .map(|(_, label)| label)
        .collect();
    let read = without_footnote_marks(&joined, &footnotes);

    let (mut citations, report) = usc::find_with_report(&read);
    let mut bare = 0;
    if let Some(default) = default {
        let found = default_code::find_bare(&read, default, &citations);
        bare = found.len();
        citations.extend(found);
    }

    let mut by_passage: Vec<(&str, Vec<UscCitation>)> = passages
        .iter()
        .map(|(path, _)| (*path, Vec::new()))
        .collect();
    for mut citation in citations {
        // The last passage that starts at or before the citation.
        let index = starts.partition_point(|start| *start <= citation.start) - 1;
        citation.start -= starts[index];
        by_passage[index].1.push(citation);
    }
    PassageCitations {
        by_passage,
        report,
        bare,
    }
}

/// A footnote mark set against the full stop after a section number, at the
/// end of a sentence: the `.6` of `31 U. S. C. §5311.6 Here we see`.
static FOOTNOTE_MARK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:§§?|\b[Ss]ec(?:tion)?s?\.?)\s*\d+[a-zA-Z]{0,4}(?P<mark>\.(?P<label>\d{1,3}))(?:\s|$)",
    )
    .expect("the footnote-mark pattern must compile")
});

/// The text with each footnote mark against a section number's full stop set
/// as spaces, so that the reader reads `§5311.6` as § 5311 (#413). No section
/// of the Code has a full stop in its number, so `5311.6` names nothing. The
/// mark is taken out only when the writing has a footnote with that label,
/// `footnotes`. The text keeps its length, so every offset stays true.
fn without_footnote_marks(text: &str, footnotes: &[&str]) -> String {
    let mut read = text.to_string();
    for found in FOOTNOTE_MARK.captures_iter(text) {
        if footnotes.contains(&&found["label"]) {
            let mark = found.name("mark").expect("the pattern names the mark");
            read.replace_range(mark.range(), &" ".repeat(mark.len()));
        }
    }
    read
}

/// One place in a passage where the court wrote a citation of a provision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Place {
    /// Where the citation starts in the words of the passage, in bytes.
    start: usize,
    /// The section as the court wrote it, brackets and all: `174(a)(1)`.
    written: String,
    /// The words the rule matched: `26 U. S. C. § 174(a)(1)`.
    words: String,
    /// The written designations that lead to no provision, from the first of
    /// them: `(e)` of `174(e)`. Absent when every one leads to a provision.
    #[serde(skip_serializing_if = "Option::is_none")]
    unresolved: Option<String>,
    /// The edition of the Code the court named after the citation, such as
    /// `1976 ed.`, and its year (#449). The object was resolved in a printing
    /// held, which is not that edition, and the section at that number can now
    /// hold another provision. Absent when the court named no edition.
    #[serde(skip_serializing_if = "Option::is_none")]
    edition: Option<usc::Edition>,
}

/// The places of one link, gathered before the link is made.
struct Gathered {
    object: String,
    title: String,
    uslm_id: String,
    places: Vec<Place>,
    /// What the rule read at each place, once each, in the order read.
    evidence: Vec<String>,
    /// Whether any place was read by the U.S.C. patterns. A place read only
    /// under the opinion's default code is read by a rule of its own.
    read_by_citation_rule: bool,
    time_basis: Option<String>,
    /// The date of the printing the object was resolved in.
    resolved_in: Option<String>,
}

impl Gathered {
    fn new(citation: &UscCitation, section: &CitedSection, object: &str, place: Place) -> Self {
        let mut gathered = Self {
            object: object.to_string(),
            title: citation.title.clone(),
            uslm_id: section.uslm_id.clone(),
            places: Vec::new(),
            evidence: Vec::new(),
            read_by_citation_rule: false,
            time_basis: None,
            resolved_in: section.resolved_in.clone(),
        };
        gathered.add(citation, place);
        gathered
    }

    fn add(&mut self, citation: &UscCitation, place: Place) {
        let evidence = evidence_of(citation);
        if !self.evidence.contains(&evidence) {
            self.evidence.push(evidence);
        }
        self.read_by_citation_rule |= citation.default_code.is_none();
        // The year the opinion says it reads the Code as in effect for, when its
        // default names one (ADR 0014, decisions 7 and 8).
        if self.time_basis.is_none() {
            self.time_basis = citation
                .default_code
                .as_ref()
                .and_then(|default| default.year.clone());
        }
        if !self.places.contains(&place) {
            self.places.push(place);
        }
    }

    /// The link: this passage cites the provision at the object.
    ///
    /// A change to its subject, object or payload must keep
    /// [`carry::Statement::of`] able to read it, or a rebuild cannot carry the
    /// reviews of the old links (#319).
    fn into_link(mut self, opinion: &Opinion) -> Link {
        self.places.sort_by_key(|place| place.start);
        let kind = LinkKind::new(LinkKind::CITES);
        let provenance = Provenance {
            source: "rule:usc_citation".to_string(),
            method: Some(match self.read_by_citation_rule {
                true => citation_rule(),
                false => default_code_rule(),
            }),
            verification: VerificationState::MachineSuggested,
            // The matched text, so a reviewer can read what the rule read. A
            // rule has no reasoning beyond the text that satisfied it, and the
            // repair it made to read that text, when it made one. Each reading
            // once, in the order of the text.
            evidence: Some(Evidence {
                reasoning: Some(self.evidence.join("; ")),
                ..Evidence::default()
            }),
            raw_score: None,
            // No clock reading. Nothing about this statement depends on when
            // the rule ran, and the same text gives the same answer on any day.
            timestamp: None,
            corroboration: None,
        };

        Link {
            subject: opinion.target(),
            // The namespace is read from the kind rather than written again, so
            // the two cannot drift apart.
            payload: Some(crate::link::KindPayload {
                namespace: kind.namespace().to_string(),
                value: json!({
                    "title": self.title,
                    // The section, whichever subdivision the object is.
                    "uslm_id": self.uslm_id,
                    // Every place in the passage where the court wrote a
                    // citation that leads to the object, in the order of the
                    // text (ADR 0014, decision 6).
                    "occurrences": self.places,
                    // Null for every citation whose opinion names no year.
                    "time_basis": self.time_basis,
                    // The date the court decided the case (ADR 0014, decision
                    // 7; #317). Null when the caller did not give one.
                    "decided": opinion.decided,
                    // The date of the printing the object was resolved in: the
                    // latest held on or before `decided`, or the earliest held
                    // when none is that early.
                    "resolved_in": self.resolved_in,
                    // True when that printing is later than the decision: the
                    // dataset holds no printing of the law the court read, so
                    // that law is out of scope.
                    "resolved_after_decision": opinion
                        .decided
                        .as_deref()
                        .zip(self.resolved_in.as_deref())
                        .map(|(decided, printed)| printed > decided),
                }),
            }),
            kind,
            object: Target::Node(self.object),
            provenance,
        }
    }
}

/// What a citation link carries as evidence: the words the rule read, and what
/// else the reading rests on.
///
/// A repair follows the words: `26 V.S.C. §§ 1361(a)(1) (OCR repair: "V.S.C."
/// read as "U.S.C.")`. A bare reference is read only because of the opinion's
/// default, so the default follows the words, with the writing that states it.
fn evidence_of(citation: &UscCitation) -> String {
    let read = match (&citation.repair, &citation.default_code) {
        (Some(repair), _) => format!("{} ({repair})", citation.text),
        (None, Some(default)) => format!(
            "{} (title {} from the default stated in {}: \"{}\")",
            citation.text, default.title, default.stated_in, default.statement
        ),
        (None, None) => citation.text.clone(),
    };
    match &citation.edition {
        Some(edition) => format!("{read} ({})", edition.written),
        None => read,
    }
}
