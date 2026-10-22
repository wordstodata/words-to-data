//! A CourtListener cluster and its opinion records, read into one stored case.
//!
//! The cluster is the case, and the case is the work: `judicial/case_<cluster>`.
//! Each writing of the case — the Court's opinion, a concurrence, a dissent — is
//! a child, `opinion_<id>`
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 1 and 2). The text is on the opinion records; the date the court
//! filed the case is on the cluster.
//!
//! Only the fields this crate reads are declared. Everything else in the
//! response is ignored rather than carried, which is a deliberate choice and not
//! an oversight: a field nobody reads, stored anyway, is a fact in a file that
//! no reader can explain.

use serde::Deserialize;
use serde_json::Value;

use super::structure::{Passage, PassageKind, Structure, read_structure};
use super::{CourtListenerError, markup::MarkupReport, markup::text_of_markup};
use crate::dataset::{Expression, ExpressionId, WorkId};
use crate::document::{DocumentNode, NodeData, NodeType, text_method};
use crate::judicial::{CaseFacts, OpinionFacts, PassageFacts};
use crate::link::{Provenance, VerificationState};
use crate::method::Method;

/// The work a case is: `judicial/case_122262`.
///
/// A structural path on the same rule as `uscode/title_26`: the class, then
/// `<kind>_<number>`. The number is the publisher's cluster id, which is the
/// only stable name the source gives a case. One case is often several opinion
/// records, so the opinion id cannot name it
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 1).
pub fn work_id(cluster_id: u64) -> WorkId {
    WorkId::new(format!("judicial/case_{cluster_id}"))
}

/// The path of one writing in its case: `judicial/case_122262/opinion_9434365`.
pub fn writing_path(cluster_id: u64, opinion_id: u64) -> String {
    format!("{}/opinion_{opinion_id}", work_id(cluster_id))
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
    /// Where this writing stands among the writings of its case: 1 is printed
    /// first. `None` when the publisher gives no order.
    #[serde(default)]
    pub ordering_key: Option<u32>,
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
    /// Harvard's head matter: the parties, docket number, dates, the judges
    /// line and the attorneys, as markup. No part of any writing.
    #[serde(default)]
    pub headmatter: Option<String>,
    /// The syllabus a reporter or the court prepared. No part of any writing.
    #[serde(default)]
    pub syllabus: Option<String>,
    /// Headnotes a publisher wrote. No part of any writing.
    #[serde(default)]
    pub headnotes: Option<String>,
    /// A summary of the case, such as a court staff summary. No part of any
    /// writing.
    #[serde(default)]
    pub summary: Option<String>,
    /// The opinion records of the case, as API addresses such as
    /// `https://www.courtlistener.com/api/rest/v4/opinions/9434364/`.
    #[serde(default)]
    pub sub_opinions: Vec<String>,
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
    /// The publisher's kind of citation, as a number: 6 is a LEXIS id and 7 a
    /// Westlaw id, which name a database record and print no page.
    #[serde(rename = "type", default)]
    pub kind: Option<u8>,
}

impl ReporterCitation {
    /// `576 U.S. 644`.
    pub fn printed(&self) -> String {
        format!("{} {} {}", self.volume, self.reporter, self.page)
    }

    /// `576 U.S.`: the volume whose pages a page label counts.
    fn volume_printed(&self) -> String {
        format!("{} {}", self.volume, self.reporter)
    }
}

/// Reporters that print the decisions of a court that has an official
/// reporter, and so are never the official one: the commercial parallels of the
/// U.S. Reports. A page of one of them is not a path
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 4: the same pin cite must mean the same path in every case).
const PARALLEL_REPORTERS: [&str; 3] = ["S. Ct.", "L. Ed.", "L. Ed. 2d"];

/// CourtListener's citation kinds that name a database record rather than a
/// printed page: LEXIS, Westlaw, a neutral citation and a journal.
const NO_PAGES: [u8; 4] = [6, 7, 8, 9];

/// The citation of the official reporter whose pages a writing's page labels
/// count, when they count one.
///
/// A label belongs to the volume the case starts in: the citation, of those
/// whose first page is at or before the first label, with the latest first
/// page. *Boeing*'s first label is 441, and of 537 U.S. 437 and 155 L. Ed. 2d
/// 17 the case is at 437, so the labels are pages of 537 U.S. *Obergefell*'s
/// first label is 2594, and its case is at 135 S. Ct. 2584, so its labels are
/// pages of a commercial reporter and count nothing here.
fn official_pages<'a>(
    first_label: &str,
    citations: &'a [ReporterCitation],
) -> Option<&'a ReporterCitation> {
    let label: u64 = first_label.parse().ok()?;
    let starts = citations
        .iter()
        .filter(|citation| !citation.kind.is_some_and(|kind| NO_PAGES.contains(&kind)))
        .filter_map(|citation| Some((citation.page.parse::<u64>().ok()?, citation)))
        .filter(|(first_page, _)| *first_page <= label)
        .max_by_key(|(first_page, _)| *first_page)?;
    let (_, citation) = starts;
    (!PARALLEL_REPORTERS.contains(&citation.reporter.as_str())).then_some(citation)
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
/// The order is by **structure**: the writings, the paragraphs with stable ids
/// and the page labels of the official reporter are in `xml_harvard` and
/// `html_columbia`, and in nothing else
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 3). It was once by fidelity of the characters, which put a court's
/// own document first and Harvard's OCR last
/// (`docs/adr/0008-an-opinions-text-is-one-named-field-chosen-for-fidelity.md`).
/// The cost of the new order is stated where it is paid: Harvard's case text is
/// raw OCR that "has not received human review", so it is `MachineSuggested`,
/// and every reader that shows it says so.
///
/// Every entry below is a field this crate has actually met in a fetched record.
const TEXT_FIELDS: [TextSource; 6] = [
    // Harvard's Caselaw Access Project. CAP states that case text "has been
    // generated by machine OCR and has not received human review", so no person
    // has confirmed that these are the court's words.
    text_field("xml_harvard", text_method::OCR, SUGGESTED, true),
    // Editor-prepared HTML from a donated collection; a person handled it. Its
    // star pagination names the pages of the reporter.
    text_field("html_columbia", text_method::MARKUP, ASSERTED, true),
    // Born-digital markup, the richest of the donated archives.
    text_field("html_anon_2020", text_method::MARKUP, ASSERTED, true),
    text_field("html_lawbox", text_method::MARKUP, ASSERTED, true),
    // A court website's HTML, or Resource.org's. Quality varies widely.
    text_field("html", text_method::MARKUP, ASSERTED, true),
    // Extracted from the document the publisher supplied — a court's PDF or Word
    // file. The characters are the publisher's own, and nothing marks a page or
    // a paragraph.
    text_field("plain_text", text_method::TEXT_LAYER, ASSERTED, false),
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
        let (source, raw) = self.chosen_field()?;
        let (text, report) = match source.markup {
            true => text_of_markup(raw),
            false => (raw.to_string(), MarkupReport::default()),
        };
        Some((text, source, report))
    }

    /// The field the text is read from, as written, and what it says.
    fn chosen_field(&self) -> Option<(TextSource, &str)> {
        let mut source = *TEXT_FIELDS
            .iter()
            .find(|candidate| self.field(candidate.field).is_some())?;
        let raw = self.field(source.field)?;

        if self.extracted_by_ocr == Some(true) {
            source.method = text_method::OCR;
            source.verification = VerificationState::MachineSuggested;
        }
        Some((source, raw))
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

    /// The ids of the case's opinion records, read off the end of each address
    /// in `sub_opinions`. An address this build cannot read is left out.
    pub fn opinion_ids(&self) -> Vec<u64> {
        self.sub_opinions
            .iter()
            .filter_map(|address| {
                address
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()?
                    .parse()
                    .ok()
            })
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

/// One case as one expression, and how the text of each writing was read.
#[derive(Debug, Clone)]
pub struct CaseExpression {
    /// The case: a `judicial.case` root with a child for the matter that is no
    /// part of any writing, such as `judicial.headmatter`, then one
    /// `judicial.opinion` child for each writing.
    pub expression: Expression,
    /// One entry for each writing, in the order of the writings.
    pub writings: Vec<WritingText>,
    /// What the pass could not read in the matter that is no part of any
    /// writing, such as the head matter. One report for each such child.
    pub matter_reports: Vec<MarkupReport>,
}

/// Where the text of one writing came from, and what the pass could not read.
///
/// The markup report is returned rather than printed, so the caller decides how
/// a character it could not decode is shown. Ignoring it is a way to lose a
/// citation quietly (`super::markup`).
#[derive(Debug, Clone)]
pub struct WritingText {
    pub opinion_id: u64,
    /// The writing's path, such as `judicial/case_122262/opinion_9434365`.
    pub path: String,
    pub source: TextSource,
    pub report: MarkupReport,
    /// The whole text of the writing, as one string: every paragraph and then
    /// every footnote, in the order the source has them, a blank line between
    /// two. What the citation reader reads, so that each citation in the
    /// writing is found once, whichever node holds its words.
    pub text: String,
}

/// The cluster fields that hold matter the source says is no part of the
/// opinion, each stored as its own child of the case, `judicial/case_<cluster>/
/// <field>`, with the node type `judicial.<field>`
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 1). In the order a reporter prints them, before the writings.
///
/// `headmatter` is Harvard's, and CAP says its "general head matter has been
/// generated by machine OCR and has not received human review", so it is
/// `MachineSuggested`. The other three are the publisher's markup.
const CASE_MATTER: [TextSource; 4] = [
    text_field("headmatter", text_method::OCR, SUGGESTED, true),
    text_field("syllabus", text_method::MARKUP, ASSERTED, true),
    text_field("headnotes", text_method::MARKUP, ASSERTED, true),
    text_field("summary", text_method::MARKUP, ASSERTED, true),
];

impl ClusterRecord {
    /// The field named, when the record carries anything in it.
    fn matter(&self, name: &str) -> Option<&str> {
        let value = match name {
            "headmatter" => &self.headmatter,
            "syllabus" => &self.syllabus,
            "headnotes" => &self.headnotes,
            "summary" => &self.summary,
            _ => return None,
        };
        value.as_deref().filter(|text| !text.trim().is_empty())
    }
}

/// The matter of a case that is no part of any writing, one node for each
/// field of [`CASE_MATTER`] the cluster fills. Its markup report is returned,
/// as a writing's is.
fn case_matter_nodes(
    cluster: &ClusterRecord,
    date: time::Date,
) -> Vec<(DocumentNode, MarkupReport)> {
    CASE_MATTER
        .iter()
        .filter_map(|source| {
            let raw = cluster.matter(source.field)?;
            let (text, report) = text_of_markup(raw);
            let provenance = Provenance {
                source: format!("courtlistener:cluster/{}:{}", cluster.id, source.field),
                method: Some(Method::new(source.method, text_method::VERSION)),
                verification: source.verification,
                evidence: None,
                raw_score: None,
                timestamp: None,
                corroboration: None,
            };
            let data = NodeData {
                content: Some(text.into()),
                ..NodeData::new(
                    format!("{}/{}", work_id(cluster.id), source.field),
                    NodeType::new(format!("{}.{}", NodeType::JUDICIAL, source.field)),
                    date,
                )
            }
            .with_provenance(provenance);
            Some((DocumentNode::leaf(data), report))
        })
        .collect()
}

/// CourtListener's type for a record that holds every writing of a case in one
/// text.
const COMBINED: &str = "010combined";

impl OpinionRecord {
    /// Whether the publisher says this record holds the whole case in one text,
    /// rather than one writing of it.
    fn is_combined(&self) -> bool {
        self.opinion_type.as_deref() == Some(COMBINED)
    }
}

/// Split the records of one case into its writings and the combined records
/// that are only sources.
///
/// When the case has split records beside a combined one, the split records
/// are the writings and the combined record is only a source, so search never
/// returns the same words twice
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 2). When the case has only a combined record, that record is the
/// one writing, and its type `010combined` says that it holds the whole case.
/// The writings are in the publisher's order, then by id.
fn writings_and_sources(opinions: &[OpinionRecord]) -> (Vec<&OpinionRecord>, Vec<u64>) {
    let split = opinions.iter().any(|opinion| !opinion.is_combined());
    let (mut writings, sources): (Vec<&OpinionRecord>, Vec<&OpinionRecord>) = opinions
        .iter()
        .partition(|opinion| !split || !opinion.is_combined());
    writings.sort_by_key(|opinion| (opinion.ordering_key.unwrap_or(u32::MAX), opinion.id));
    let mut sources: Vec<u64> = sources.iter().map(|opinion| opinion.id).collect();
    sources.sort();
    (writings, sources)
}

/// A case as one expression of one work: the cluster is the root, and each
/// writing is a child.
///
/// `opinions` are the publisher's records for the cluster — all of them, so
/// that the case is whole and a combined record is known to be only a source.
/// A record of another cluster is refused rather than filed under this case.
///
/// The date is `cluster.date_filed`, the day the court filed the case. An
/// opinion record's own `date_created` is the day CourtListener ingested it,
/// and using it would date every expression in the dataset to the day a third
/// party scraped it, which is the field that looks right.
pub fn case_expression(
    cluster: &ClusterRecord,
    opinions: &[OpinionRecord],
) -> Result<CaseExpression, CourtListenerError> {
    let incomplete = |field| CourtListenerError::Incomplete {
        kind: "cluster",
        id: cluster.id,
        field,
    };
    let date_filed = cluster
        .date_filed
        .as_deref()
        .ok_or_else(|| incomplete("date_filed"))?;
    let date = crate::date::date_str_to_date(date_filed)
        .map_err(|error| CourtListenerError::Http(error.to_string()))?;
    let case_name = cluster
        .case_name
        .clone()
        .ok_or_else(|| incomplete("case_name"))?;

    if let Some(stranger) = opinions
        .iter()
        .find(|opinion| opinion.cluster_id.is_some_and(|id| id != cluster.id))
    {
        return Err(CourtListenerError::Incomplete {
            kind: "opinion",
            id: stranger.id,
            field: "this cluster's id",
        });
    }

    let (writing_records, combined_records) = writings_and_sources(opinions);
    let mut children = Vec::new();
    let mut matter_reports = Vec::new();
    for (node, report) in case_matter_nodes(cluster, date) {
        children.push(node);
        matter_reports.push(report);
    }
    let mut writings = Vec::new();
    for opinion in writing_records {
        let path = writing_path(cluster.id, opinion.id);
        let (node, source, report, text) = writing_node(opinion, &path, date, &cluster.citations)?;
        children.push(node);
        writings.push(WritingText {
            opinion_id: opinion.id,
            path,
            source,
            report,
            text,
        });
    }

    let facts = CaseFacts {
        case_name: case_name.clone(),
        cluster_id: cluster.id,
        panel: cluster
            .judges
            .clone()
            .filter(|judges| !judges.trim().is_empty()),
        precedential_status: cluster.precedential_status.clone(),
        citations: cluster.printed_citations(),
        source: cluster.source.clone(),
        docket_id: cluster.docket_id,
        combined_records,
    };

    let work = work_id(cluster.id);
    let root = NodeData {
        // The case name is what a reader sees when the case is listed, so it is
        // the node's heading. It stays in the payload too, under the name a
        // judicial reader asks for it by.
        heading: Some(case_name.into()),
        ..NodeData::new(work.as_str(), NodeType::new(NodeType::CASE), date)
    }
    .with_payload(facts.to_payload().map_err(CourtListenerError::from)?);

    Ok(CaseExpression {
        expression: Expression {
            id: ExpressionId::new(work, date_filed),
            label: None,
            root: DocumentNode {
                data: root,
                children,
            },
        },
        writings,
        matter_reports,
    })
}

/// The fields whose markup marks paragraphs and the reporter's pages, so that
/// a writing read from one of them is stored as its paragraphs and footnotes
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 4).
const STRUCTURED: [&str; 2] = ["xml_harvard", "html_columbia"];

/// One writing as a node at `path`: its text, where the text came from, what
/// the publisher says about the writing, and its whole text as one string.
///
/// A writing read from a field in [`STRUCTURED`] holds no text itself. Its
/// paragraphs and footnotes are its children, each with the same provenance,
/// so a search hit names the paragraph that holds the words.
fn writing_node(
    opinion: &OpinionRecord,
    path: &str,
    date: time::Date,
    citations: &[ReporterCitation],
) -> Result<(DocumentNode, TextSource, MarkupReport, String), CourtListenerError> {
    let (source, raw) = opinion
        .chosen_field()
        .ok_or(CourtListenerError::Incomplete {
            kind: "opinion",
            id: opinion.id,
            field: "any text field",
        })?;

    let facts = OpinionFacts {
        opinion_type: opinion.opinion_type.clone(),
        author: opinion
            .author_str
            .clone()
            .filter(|author| !author.trim().is_empty()),
        per_curiam: opinion.per_curiam,
        sha1: opinion.sha1.clone(),
        page_count: opinion.page_count,
        download_url: opinion.download_url.clone(),
        pages: None,
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

    let writing = |facts: OpinionFacts| -> Result<NodeData, CourtListenerError> {
        Ok(NodeData::new(path, NodeType::new(NodeType::OPINION), date)
            .with_provenance(provenance.clone())
            .with_payload(facts.to_payload()?))
    };

    if !STRUCTURED.contains(&source.field) {
        let (text, report) = match source.markup {
            true => text_of_markup(raw),
            false => (raw.to_string(), MarkupReport::default()),
        };
        let data = NodeData {
            content: Some(text.clone().into()),
            ..writing(facts)?
        };
        return Ok((DocumentNode::leaf(data), source, report, text));
    }

    let structure = read_structure(raw);
    let pages = structure
        .first_page_mark
        .as_deref()
        .and_then(|label| official_pages(label, citations));
    let structure = match pages {
        Some(_) => structure,
        None => structure.without_pages(),
    };
    let facts = OpinionFacts {
        pages: pages.map(ReporterCitation::volume_printed),
        ..facts
    };
    let text = structure
        .passages
        .iter()
        .map(|passage| passage.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let children = passage_nodes(&structure, path, date, &provenance)?;
    let node = DocumentNode {
        data: writing(facts)?,
        children,
    };
    Ok((node, source, structure.report, text))
}

/// The paragraphs of a writing under the pages they start on, then its
/// footnotes, as nodes.
///
/// `page_<p>/paragraph_<n>`, where `n` counts the paragraphs that start on page
/// `p`, or `paragraph_<n>` alone when the writing has no pages; and
/// `footnote_<label>`.
fn passage_nodes(
    structure: &Structure,
    writing: &str,
    date: time::Date,
    provenance: &Provenance,
) -> Result<Vec<DocumentNode>, CourtListenerError> {
    let leaf = |path: String, node_type: &str, passage: &Passage| {
        let facts = PassageFacts {
            source_ids: passage.source_ids.clone(),
        };
        let data = NodeData {
            content: Some(passage.text.as_str().into()),
            ..NodeData::new(path, NodeType::new(node_type), date)
        }
        .with_provenance(provenance.clone())
        .with_payload(facts.to_payload()?);
        Ok::<_, CourtListenerError>(DocumentNode::leaf(data))
    };

    // Every page printed, whether or not a paragraph starts on it.
    let mut children: Vec<DocumentNode> = structure
        .pages
        .iter()
        .map(|page| {
            DocumentNode::leaf(NodeData::new(
                format!("{writing}/page_{page}"),
                NodeType::new(NodeType::PAGE),
                date,
            ))
        })
        .collect();
    let mut footnotes = Vec::new();
    let mut unpaged = 0;
    for passage in &structure.passages {
        match &passage.kind {
            PassageKind::Footnote { label } => footnotes.push(leaf(
                format!("{writing}/footnote_{label}"),
                NodeType::FOOTNOTE,
                passage,
            )?),
            PassageKind::Paragraph { page: None } => {
                unpaged += 1;
                children.push(leaf(
                    format!("{writing}/paragraph_{unpaged}"),
                    NodeType::PARAGRAPH,
                    passage,
                )?);
            }
            PassageKind::Paragraph { page: Some(page) } => {
                let page_path = format!("{writing}/page_{page}");
                let index = match children
                    .iter()
                    .position(|child| *child.data.path == *page_path)
                {
                    Some(index) => index,
                    None => {
                        children.push(DocumentNode::leaf(NodeData::new(
                            page_path.as_str(),
                            NodeType::new(NodeType::PAGE),
                            date,
                        )));
                        children.len() - 1
                    }
                };
                let page_node = &mut children[index];
                let n = page_node.children.len() + 1;
                page_node.children.push(leaf(
                    format!("{page_path}/paragraph_{n}"),
                    NodeType::PARAGRAPH,
                    passage,
                )?);
            }
        }
    }
    children.extend(footnotes);
    Ok(children)
}
