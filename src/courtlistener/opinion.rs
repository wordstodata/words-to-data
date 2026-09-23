//! A CourtListener opinion and its cluster, read into one stored node.
//!
//! Two records, because the two facts an expression needs sit on different ones.
//! The text is on the opinion; the date the court filed it is on the cluster.
//!
//! Only the fields this crate reads are declared. Everything else in the
//! response is ignored rather than carried, which is a deliberate choice and not
//! an oversight: a field nobody reads, stored anyway, is a fact in a file that
//! no reader can explain.

use serde::Deserialize;
use serde_json::Value;

use super::{CourtListenerError, markup::MarkupReport, markup::text_of_markup};
use crate::dataset::{Expression, ExpressionId, WorkId};
use crate::document::{DocumentNode, NodeData, NodeType, text_method};
use crate::method::Method;
use crate::judicial::OpinionFacts;
use crate::link::{Provenance, VerificationState};

/// The work an opinion is the single expression of: `judicial/opinion_2812209`.
///
/// A structural path on the same rule as `uscode/title_26`: the class, then
/// `<kind>_<number>`. The number is the publisher's opinion id, which is the
/// only stable name the source gives.
///
/// #53 suggested `courtlistener/opinion_<id>`. That names the publisher rather
/// than the class, and `docs/adr/0006-a-document-node-is-class-neutral.md` puts
/// the class in the first segment — a node's type is `judicial.opinion`, and the
/// path and the type must not disagree about what a thing is. Two publishers of
/// the same opinion are two ids under one class, not two classes.
pub fn work_id(opinion_id: u64) -> WorkId {
    WorkId::new(format!("judicial/opinion_{opinion_id}"))
}

/// One CourtListener opinion record: the writing itself.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpinionRecord {
    pub id: u64,
    #[serde(default)]
    pub cluster_id: Option<u64>,
    /// The publisher's own vocabulary, such as `010combined` or `040dissent`.
    #[serde(rename = "type", default)]
    pub opinion_type: Option<String>,
    #[serde(default)]
    pub author_str: Option<String>,
    #[serde(default)]
    pub per_curiam: Option<bool>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub page_count: Option<u32>,
    /// Where the publisher says it got the document, such as a court's PDF.
    #[serde(default)]
    pub download_url: Option<String>,
    /// Whether a machine read the document rather than a person or a text layer.
    ///
    /// CourtListener sets this on **every** opinion imported from Harvard, so
    /// for two thirds of the corpus it is a statement about the import route
    /// rather than a measurement of one document
    /// (`docs/research/courtlistener-formats.md`, section 2).
    #[serde(default)]
    pub extracted_by_ocr: Option<bool>,

    // The text fields, in no particular order here; [`TEXT_FIELDS`] holds the
    // order they are preferred in and says why.
    #[serde(default)]
    pub plain_text: Option<String>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub html_lawbox: Option<String>,
    #[serde(default)]
    pub html_columbia: Option<String>,
    #[serde(default)]
    pub html_anon_2020: Option<String>,
    #[serde(default)]
    pub xml_harvard: Option<String>,
}

/// One CourtListener cluster record: the case the writing belongs to.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClusterRecord {
    pub id: u64,
    /// **The court's date.** Not `date_created`, which is when CourtListener
    /// ingested the record.
    #[serde(default)]
    pub date_filed: Option<String>,
    #[serde(default)]
    pub case_name: Option<String>,
    #[serde(default)]
    pub precedential_status: Option<String>,
    /// The publisher's provenance letters, such as `CU`: a court website and
    /// Harvard, merged.
    #[serde(default)]
    pub source: Option<String>,
    /// The judges, in the publisher's own words and punctuation.
    #[serde(default)]
    pub judges: Option<String>,
    #[serde(default)]
    pub docket_id: Option<u64>,
    #[serde(default)]
    pub citations: Vec<ReporterCitation>,
}

/// One parallel citation, as a reporter prints it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReporterCitation {
    #[serde(default)]
    pub volume: String,
    #[serde(default)]
    pub reporter: String,
    /// Text, never a number. Nebraska prints Roman numerals here, Connecticut
    /// prints `13301-M` (`docs/research/courtlistener-formats.md`, section 5).
    #[serde(default)]
    pub page: String,
}

impl ReporterCitation {
    /// `576 U.S. 644`.
    pub fn printed(&self) -> String {
        format!("{} {} {}", self.volume, self.reporter, self.page)
    }
}

/// Where an opinion's text came from, and what that says about trusting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSource {
    /// The record field the text was taken from, such as `xml_harvard`.
    pub field: &'static str,
    /// How the text was obtained: one of [`text_method`].
    pub method: &'static str,
    /// How far the text can be trusted to be the words the court wrote.
    pub verification: VerificationState,
    /// Whether the field held markup that had to be stripped.
    pub markup: bool,
}

/// A field carrying OCR output is a machine's proposal about what a page says.
const SUGGESTED: VerificationState = VerificationState::MachineSuggested;

/// Anything else is the publisher asserting its own characters.
const ASSERTED: VerificationState = VerificationState::Asserted;

/// The text fields, in the order this crate prefers them, with what each says.
///
/// The order is by **fidelity of the characters**, not by richness of the
/// markup. `docs/research/courtlistener-formats.md` ranks the fields the other
/// way round, putting `xml_harvard` near the top, and it is right to: it is
/// ranking them for a reader that wants structure. This crate stores an opinion
/// as one node and parses no structure (#53), so the only question left is
/// whether the characters are the ones the court printed. On that question
/// Harvard's field is the worst of them, not the best — its case text is raw OCR
/// that "has not received human review" — and a court's own document is the best.
///
/// The whole decision, with what was rejected, is
/// `docs/adr/0008-an-opinions-text-is-one-named-field-chosen-for-fidelity.md`.
/// Every entry below is a field this crate has actually met in a fetched record.
const TEXT_FIELDS: [TextSource; 6] = [
    // Extracted from the document the publisher supplied — a court's PDF or Word
    // file. The characters are the publisher's own.
    text_field("plain_text", text_method::TEXT_LAYER, ASSERTED, false),
    // Born-digital markup, the richest of the donated archives.
    text_field("html_anon_2020", text_method::MARKUP, ASSERTED, true),
    // Editor-prepared HTML from a donated collection; a person handled it.
    text_field("html_columbia", text_method::MARKUP, ASSERTED, true),
    text_field("html_lawbox", text_method::MARKUP, ASSERTED, true),
    // A court website's HTML, or Resource.org's. Quality varies widely.
    text_field("html", text_method::MARKUP, ASSERTED, true),
    // Harvard's Caselaw Access Project. CAP states that case text "has been
    // generated by machine OCR and has not received human review", so no person
    // has confirmed that these are the court's words.
    text_field("xml_harvard", text_method::OCR, SUGGESTED, true),
];

/// One entry of [`TEXT_FIELDS`], so the table reads as a table.
const fn text_field(
    field: &'static str,
    method: &'static str,
    verification: VerificationState,
    markup: bool,
) -> TextSource {
    TextSource {
        field,
        method,
        verification,
        markup,
    }
}

impl OpinionRecord {
    /// Read one opinion out of a response.
    pub fn from_json(json: &str) -> Result<Self, CourtListenerError> {
        Ok(serde_json::from_value(sole_record(json)?)?)
    }

    /// The field named, when the record carries anything in it.
    fn field(&self, name: &str) -> Option<&str> {
        let value = match name {
            "plain_text" => &self.plain_text,
            "html_anon_2020" => &self.html_anon_2020,
            "html_columbia" => &self.html_columbia,
            "html_lawbox" => &self.html_lawbox,
            "html" => &self.html,
            "xml_harvard" => &self.xml_harvard,
            _ => return None,
        };
        value.as_deref().filter(|text| !text.trim().is_empty())
    }

    /// The opinion's text, where it came from, and what the pass could not read.
    ///
    /// `None` means the record carries no text in any field this crate reads,
    /// which is a record with nothing in it rather than an empty opinion.
    ///
    /// A record that says a machine read the document is downgraded whatever
    /// field the text came from: the publisher has told us the characters are a
    /// machine's reading, and taking the field's own word over that would be
    /// claiming more than we were told.
    pub fn text(&self) -> Option<(String, TextSource, MarkupReport)> {
        let mut source = *TEXT_FIELDS
            .iter()
            .find(|candidate| self.field(candidate.field).is_some())?;
        let raw = self.field(source.field)?;

        if self.extracted_by_ocr == Some(true) {
            source.method = text_method::OCR;
            source.verification = VerificationState::MachineSuggested;
        }

        let (text, report) = match source.markup {
            true => text_of_markup(raw),
            false => (raw.to_string(), MarkupReport::default()),
        };
        Some((text, source, report))
    }
}

impl ClusterRecord {
    /// Read one cluster out of a response.
    pub fn from_json(json: &str) -> Result<Self, CourtListenerError> {
        Ok(serde_json::from_value(sole_record(json)?)?)
    }

    /// How the reporters print this case: `576 U.S. 644`, `135 S. Ct. 2584`.
    pub fn printed_citations(&self) -> Vec<String> {
        self.citations
            .iter()
            .map(ReporterCitation::printed)
            .collect()
    }

    /// What to call the case on a page: `Snow v. Commissioner, 416 U.S. 500`.
    ///
    /// The first parallel citation only. A lawyer names a case by one of them,
    /// and a display string carrying five is not a name.
    pub fn display(&self) -> String {
        let name = self.case_name.as_deref().unwrap_or("Unnamed case");
        match self.citations.first() {
            Some(citation) => format!("{name}, {}", citation.printed()),
            None => name.to_string(),
        }
    }
}

/// One record out of a response, whichever shape it arrived in.
///
/// `GET /opinions/<id>/` answers with the record itself. The list form,
/// `GET /opinions/?id=<id>`, wraps it in `results`, and the first cached record
/// this work started from was taken that way. Both are read, because both are on
/// disk, and re-fetching one to tidy its shape would spend a request from a
/// budget of 125 a day to change nothing.
fn sole_record(json: &str) -> Result<Value, CourtListenerError> {
    let value: Value = serde_json::from_str(json)?;
    match value.get("results").and_then(Value::as_array) {
        Some(results) => Ok(results.first().cloned().unwrap_or(Value::Null)),
        None => Ok(value),
    }
}

/// The opinion as one expression of one work: one node, no children.
///
/// Both records are required, and that is the point of the signature. The date
/// is `cluster.date_filed` — the day the court filed the opinion — and the
/// opinion record's own `date_created` is the day CourtListener ingested it.
/// Using the second would date every expression in the dataset to the day a
/// third party scraped it, which is the field that looks right.
///
/// The markup report is returned rather than printed, so the caller decides how
/// a character it could not decode is shown. Ignoring it is a way to lose a
/// citation quietly (`super::markup`).
pub fn opinion_expression(
    opinion: &OpinionRecord,
    cluster: &ClusterRecord,
) -> Result<(Expression, TextSource, MarkupReport), CourtListenerError> {
    let missing = |field| CourtListenerError::Incomplete {
        kind: "opinion",
        id: opinion.id,
        field,
    };

    let date_filed = cluster
        .date_filed
        .as_deref()
        .ok_or(CourtListenerError::Incomplete {
            kind: "cluster",
            id: cluster.id,
            field: "date_filed",
        })?;
    let date = crate::date::date_str_to_date(date_filed)
        .map_err(|error| CourtListenerError::Http(error.to_string()))?;

    let (text, source, report) = opinion.text().ok_or_else(|| missing("any text field"))?;

    let facts = OpinionFacts {
        case_name: cluster
            .case_name
            .clone()
            .ok_or_else(|| missing("case_name"))?,
        opinion_type: opinion.opinion_type.clone(),
        author: opinion
            .author_str
            .clone()
            .filter(|author| !author.trim().is_empty()),
        per_curiam: opinion.per_curiam,
        panel: cluster
            .judges
            .clone()
            .filter(|judges| !judges.trim().is_empty()),
        precedential_status: cluster.precedential_status.clone(),
        citations: cluster.printed_citations(),
        source: cluster.source.clone(),
        sha1: opinion.sha1.clone(),
        page_count: opinion.page_count,
        cluster_id: opinion.cluster_id.or(Some(cluster.id)),
        docket_id: cluster.docket_id,
        download_url: opinion.download_url.clone(),
    };

    // Where the text came from is core provenance, not a judicial fact: a reader
    // deciding whether to rely on a passage must not have to open an extension
    // payload to learn that a machine read it off a scan
    // (`docs/adr/0006-a-document-node-is-class-neutral.md`).
    let provenance = Provenance {
        // The publisher, the record, and the field, so a reader can go and look
        // at the same text we read.
        source: format!("courtlistener:opinion/{}:{}", opinion.id, source.field),
        method: Some(Method::new(source.method, text_method::VERSION)),
        verification: source.verification,
        evidence: None,
        raw_score: None,
        // No clock reading. The statement is about which field of a fixed
        // record the text came from, and that does not depend on when we asked.
        timestamp: None,
        corroboration: None,
    };

    let work = work_id(opinion.id);
    let data = NodeData {
        // The case name is what a reader sees when the opinion is listed, so it
        // is the node's heading. It stays in the payload too, under the name a
        // judicial reader asks for it by.
        heading: Some(facts.case_name.clone().into()),
        content: Some(text.into()),
        ..NodeData::new(work.as_str(), NodeType::new(NodeType::OPINION), date)
    }
    .with_provenance(provenance)
    .with_payload(facts.to_payload().map_err(CourtListenerError::from)?);

    Ok((
        Expression {
            id: ExpressionId::new(work, date_filed),
            label: None,
            root: DocumentNode::leaf(data),
        },
        source,
        report,
    ))
}
