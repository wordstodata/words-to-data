//! `words_to_data path` — everything about one structural path: where it
//! exists, its field-level changes for an expression pair, and its annotations.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect;
use words_to_data::judicial::cites::{CitationLink, PathRole};
use words_to_data::judicial::held::{CourtFacts, HeldWriting};
use words_to_data::query::{DEFAULT_LIMIT, Locator, PathMatch};

use crate::load::{self, with_dataset};

/// Where a reader of `path` finds the words a screen left out.
const EVERY_WORD: &str = "--json carries every field in full.";

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Structural path to inspect (e.g. `uscode/title_9/chapter_1/section_1`).
    /// Annotations on this path and on every path beneath it are reported
    pub path: String,

    /// Older expression for field changes, e.g. `uscode/title_9@2025-07-18` (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<ExpressionId>,

    /// Newer expression of the same work (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<ExpressionId>,

    /// Report only the annotations recorded on this path itself, not those beneath it
    #[arg(long)]
    pub exact: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(mut args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let pair = match (&args.from, &args.to) {
        (Some(from), Some(to)) => {
            crate::held::refuse_unless_window_held(&ds, from, to);
            Some((from, to))
        }
        _ => None,
    };
    args.path = with_dataset!(&ds, d => crate::held::path_argument(d, &args.path));

    let matching = crate::annotations::path_matching(args.exact);
    let report = crate::fail::or_exit(
        with_dataset!(&ds, d => inspect::path_report(d, &args.path, pair, matching)),
        "Error building path report",
    );
    // Asked only when no expression holds the path. "(no expression)" alone
    // sent an agent to SQL to find the real path (#326). "Present in" is about
    // the path itself, so the question is too.
    let absent = if report.present_in.is_empty() {
        let locator = Locator::new().at_path(&args.path, PathMatch::Exact);
        crate::fail::or_exit(
            with_dataset!(&ds, d => inspect::absent_path(d, &locator)),
            "Error checking the path",
        )
    } else {
        None
    };
    // A path that no expression holds and nothing names is not held: the run
    // refuses it, as `diff` does (#392, #425). A link, a citation or a
    // classification can name a path no expression holds, and then the report
    // is a true answer and is printed with the notice.
    let names_it = !report.annotations.is_empty()
        || !report.citations.is_empty()
        || !report.olrc_classifications.is_empty()
        || !report.unfollowed_redesignations.is_empty();
    if let Some(absent) = &absent
        && !names_it
    {
        crate::fail::refuse(&crate::annotations::absent_notice(
            absent,
            args.exact,
            &crate::held::scope_of(None, None),
        ));
    }

    if args.json {
        // Only when the path is absent, so the answer over a real path is the
        // same as it was. A `Value` sorts its keys, so the report is printed
        // as itself when there is nothing to add.
        let answer = match &absent {
            None => serde_json::to_string_pretty(&report),
            Some(absent) => {
                let mut answer = serde_json::json!(report);
                answer["absent_path"] = serde_json::json!(absent);
                serde_json::to_string_pretty(&answer)
            }
        };
        println!("{}", answer.unwrap());
        return;
    }

    println!("PATH: {}", report.path);
    println!(
        "Present in: {}",
        if report.present_in.is_empty() {
            "(no expression)".to_string()
        } else {
            report
                .present_in
                .iter()
                .map(|p| format!("{} ({})", p.expression, provisions(p.provisions)))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    if let Some(nearest) = absent.as_ref().and_then(|a| a.nearest_held.as_ref()) {
        println!("Nearest held path: {nearest}");
    }

    if let Some(facts) = &report.judicial {
        print_court_facts(facts);
    }

    if let (Some(from), Some(to)) = (&args.from, &args.to) {
        println!(
            "\nProvisions ({from} -> {to}) ({}):",
            report.provisions.len()
        );
        for p in &report.provisions {
            println!("  {} — {}", verdict(&p.presence), positions(p));
            for link in &p.via {
                println!("      {}", stated_by(link));
            }
            for c in &p.changes {
                println!("      {}: {:?} -> {:?}", c.field, c.old_value, c.new_value);
            }
            match p.presence {
                inspect::Presence::Added => {
                    print_words("added", &report.path, &p.words, "      ", EVERY_WORD)
                }
                inspect::Presence::Removed => {
                    print_words("removed", &report.path, &p.words, "      ", EVERY_WORD)
                }
                _ => {}
            }
        }
    }

    print_contents(&report.contents);

    print_unfollowed(&report.unfollowed_redesignations);

    // Say which paths the count covers. A bare "Annotations (0)" reads as
    // "nothing was attributed here", which is a different statement.
    let scope = if args.exact {
        "on this path exactly"
    } else {
        "at or beneath this path"
    };
    println!("\nAnnotations {scope} ({}):", report.annotations.len());
    for a in &report.annotations {
        crate::annotations::print_annotation(a);
    }

    if let Some(first) = report.olrc_classifications.first() {
        print_classifications(&first.section, &report.olrc_classifications);
    }

    print_citations(&report.citations, report.citations.len());
}

const NOT_RECORDED: &str = "not recorded";

/// The stored facts of a writing and of its case, one named fact to a line
/// (#327).
///
/// Every fact is printed, and a missing one says "not recorded", so a reader
/// does not take a blank for an answer, nor go to SQL to find out.
fn print_court_facts(facts: &CourtFacts) {
    if let Some(writing) = &facts.writing {
        println!("\nWriting {}:", writing.path);
        print_writing_facts(writing);
    }

    let case = &facts.case;
    println!("\nCase {}@{}:", case.path, case.date);
    fact("case name", case.case_name.as_deref());
    fact(
        "cluster",
        case.cluster_id.map(|id| id.to_string()).as_deref(),
    );
    fact("panel", case.panel.as_deref());
    fact("precedential status", case.precedential_status.as_deref());
    fact("citations", listed(&case.citations, "; ").as_deref());
    fact("source", case.source.as_deref());
    fact("docket", case.docket_id.map(|id| id.to_string()).as_deref());
    fact("court", case.court.as_deref());
    // A combined record holds the same words as the writings, so it is kept
    // as a source and not as text (ADR 0014, decision 2). Its absence from
    // the writings is not a loss.
    let combined: Vec<String> = case.combined_records.iter().map(u64::to_string).collect();
    fact(
        "combined records kept as a source, not as text",
        Some(listed(&combined, ", ").as_deref().unwrap_or("none")),
    );

    // Each writing once, short, so a reader sees which of them is the Court's
    // and which a dissent. The path of a writing gives it in full.
    println!("\nWritings of this case ({}):", case.writings.len());
    for writing in &case.writings {
        println!(
            "  {}  {} · author {} · per curiam {}",
            writing.path,
            writing
                .opinion_type
                .as_deref()
                .unwrap_or("writing type not recorded"),
            writing.author.as_deref().unwrap_or(NOT_RECORDED),
            yes_or_no(writing.per_curiam).unwrap_or(NOT_RECORDED),
        );
    }
}

/// Every stored fact of one writing, and where its text came from.
fn print_writing_facts(writing: &HeldWriting) {
    fact("writing type", writing.opinion_type.as_deref());
    fact("author", writing.author.as_deref());
    fact("per curiam", yes_or_no(writing.per_curiam));
    fact(
        "page count",
        writing.page_count.map(|n| n.to_string()).as_deref(),
    );
    fact("download URL", writing.download_url.as_deref());
    fact("sha1", writing.sha1.as_deref());
    let text_from = writing
        .text_source
        .as_ref()
        .map(|source| match &writing.text_method {
            Some(method) => format!("{source} ({method})"),
            None => source.clone(),
        });
    fact("text from", text_from.as_deref());
    if let Some(found_by) = &writing.found_by {
        fact("found by", Some(found_by));
    }
    // A writing with no page level has no reporter volume, and that is a
    // fact about the source, so it is said rather than left blank (#314).
    fact(
        "pages",
        Some(
            writing
                .pages
                .as_deref()
                .unwrap_or("none: its paths are paragraph_<n>, or it is one node"),
        ),
    );
}

/// A stored yes-or-no fact in words, or `None` when it is not recorded.
fn yes_or_no(value: Option<bool>) -> Option<&'static str> {
    value.map(|value| if value { "yes" } else { "no" })
}

/// One named fact on its own line, or "not recorded".
fn fact(name: &str, value: Option<&str>) {
    println!("  {name}: {}", value.unwrap_or(NOT_RECORDED));
}

/// The values joined, or `None` when there are none.
fn listed(values: &[String], separator: &str) -> Option<String> {
    (!values.is_empty()).then(|| values.join(separator))
}

/// What is directly inside a court path: each page with how many paragraphs
/// start on it, and each paragraph or footnote with its length (#314).
fn print_contents(contents: &[inspect::ContentsEntry]) {
    if contents.is_empty() {
        return;
    }
    println!("\nInside this path ({}):", contents.len());
    for entry in contents {
        let size = match entry.node_type.as_str() {
            "judicial.page" => format!("{} paragraph(s)", entry.children),
            _ if entry.children > 0 => format!("{} part(s)", entry.children),
            _ => format!("{} chars", entry.chars),
        };
        println!("  {}  {}  {size}", entry.path, entry.node_type);
    }
    println!(
        "  A paragraph belongs to the page it starts on. Read one with \
         `search <dataset> \"\" --path <path> --exact --snippet 0`."
    );
}

/// The citing opinion as a line names it, and whether the dataset holds it.
fn opinion_of(citation: &CitationLink) -> String {
    if citation.opinion_held {
        citation.opinion.clone()
    } else {
        format!("{} (not held in this dataset)", citation.opinion)
    }
}

/// What to call a provision that holds the path asked about: a citation
/// resolves to a section, so that is the usual word.
fn holder(path: &str) -> &'static str {
    let last = path.rsplit('/').next().unwrap_or(path);
    if last.starts_with("section_") {
        "section"
    } else {
        "provision"
    }
}

/// The `judicial.cites` links that name the path, one line each (#301).
///
/// `total` is how many name it, which is more than `citations` holds when a
/// caller stopped at a limit. The caller says what it left out.
pub fn print_citations(citations: &[CitationLink], total: usize) {
    if total == 0 {
        return;
    }
    // Rows about a provision above the path are counted here too, so the
    // heading says so: each of those rows says it is not about this path.
    println!(
        "\nCitations (judicial.cites) naming this path or a provision that holds it ({total}):"
    );
    for citation in citations {
        let words = citation
            .matched_words
            .as_deref()
            .map(|words| format!(" as \"{words}\""))
            .unwrap_or_default();
        let said = match citation.role {
            // The writing is named on every line: on a case, the lines of a
            // dissent and of the Court's opinion must not read alike (#313).
            PathRole::CitingOpinion => {
                format!("{} cites {}{words}", citation.opinion, citation.cited_path)
            }
            PathRole::CitedPath => format!("cited by {}{words}", opinion_of(citation)),
            PathRole::CitedBeneathPath => format!(
                "cited by {}{words}, at {}",
                opinion_of(citation),
                citation.cited_path
            ),
            PathRole::CitedAbovePath => format!(
                "{}{words} cites the {} that holds this path, not this path: {}",
                opinion_of(citation),
                holder(&citation.cited_path),
                citation.cited_path
            ),
        };
        println!(
            "  {said} ({}) [link {}]",
            trust_of(citation.verification, citation.review.as_ref()),
            citation.id
        );
        if !citation.unresolved.is_empty() {
            println!(
                "    {} of what the court wrote leads to no provision in the printing \
                 the link was made against, so the link ends at {}",
                citation.unresolved.join(", "),
                citation.cited_path
            );
        }
        if let Some(stub) = &citation.stub_heading {
            println!(
                "    the cited {} is now a stub: {stub}. That is its heading in the latest \
                 printing held, which holds no law at that number.",
                holder(&citation.cited_path)
            );
        }
    }
}

/// The OLRC's classifications of one section, one line each.
///
/// A row about a note or a preceding heading says so on its own line. The
/// table records both against the section, and a reviewer who read one as a
/// statement about the section's text would confirm the wrong thing.
pub fn print_classifications(section: &str, rows: &[inspect::OlrcClassification]) {
    println!("\nOLRC classifications of {section} ({}):", rows.len());
    for row in rows {
        let said: Vec<&str> = row
            .descriptions
            .iter()
            .map(|d| {
                if d.trim().is_empty() {
                    "amended"
                } else {
                    d.as_str()
                }
            })
            .collect();
        let part = match row.classifies {
            inspect::ClassifiedPart::SectionText => "",
            inspect::ClassifiedPart::Note => " (a note, not the section's text)",
            inspect::ClassifiedPart::Heading => {
                " (a heading before the section, not the section's text)"
            }
            inspect::ClassifiedPart::NoteOrHeading => {
                " (a note or a heading before the section, not the section's text)"
            }
        };
        println!(
            "  classified by the OLRC from Pub. L. {} §{}, {}{part}",
            row.public_law,
            row.law_section,
            said.join("; ")
        );
    }
}

fn provisions(n: usize) -> String {
    if n == 1 {
        "1 provision".to_string()
    } else {
        format!("{n} provisions")
    }
}

fn verdict(presence: &inspect::Presence) -> String {
    match presence {
        inspect::Presence::InBoth => "in both".to_string(),
        inspect::Presence::Added => "added".to_string(),
        inspect::Presence::Removed => "removed".to_string(),
        // The destination is named here rather than left to a second command.
        // A move whose other end is not shown is the answer this report exists
        // to replace.
        inspect::Presence::MovedOut { to_path } => format!("moved out to {to_path}"),
        inspect::Presence::MovedIn { from_path } => format!("moved in from {from_path}"),
    }
}

/// Who said a provision moved, when, and how far it can be trusted.
///
/// Every link is followed whatever its corroboration figure, so the state and
/// the provenance are printed beside the claim instead. Corroboration is
/// evidence for a reviewer, not a substitute for one.
fn stated_by(link: &inspect::RedesignationLink) -> String {
    let bill = match &link.bill_id {
        Some(bill) => format!("stated by {bill}"),
        None => "stated by an unnamed source".to_string(),
    };
    let outdated = link
        .outdated
        .as_ref()
        .map(|outdated| {
            format!(
                " [outdated: made by @{}, not made again by @{}]",
                outdated.made_by.version, outdated.remade_by.version
            )
        })
        .unwrap_or_default();
    format!(
        "{bill}, between {} and {} ({}) [{}]{outdated}",
        link.from_date,
        link.to_date,
        trust(link),
        link.id
    )
}

/// How far the claim can be trusted: what the link's maker recorded, and what a
/// reviewer said about it afterwards.
///
/// Both, because they are different facts. The state is the maker's own, and
/// the review is somebody else's verdict on it. Printing only the state is what
/// made a dataset holding a refutation read exactly like one holding none
/// (#227), and printing only the review would hide that nobody has reviewed an
/// unconfirmed link at all.
fn trust(link: &inspect::RedesignationLink) -> String {
    trust_of(link.verification, link.review.as_ref())
}

/// The maker's state and the newest review, as [`trust`] prints them, for a
/// link of any kind.
fn trust_of(
    verification: words_to_data::link::VerificationState,
    review: Option<&words_to_data::review::Review>,
) -> String {
    let recorded = stored_trust(verification);
    match review {
        None => recorded.to_string(),
        Some(review) => format!(
            "{recorded}, {} by {} on {}",
            review.verdict,
            review.reviewer,
            review.at.date()
        ),
    }
}

fn stored_trust(verification: words_to_data::link::VerificationState) -> &'static str {
    use words_to_data::link::VerificationState as V;
    match verification {
        V::Asserted => "asserted by a source",
        V::MachineSuggested => "machine suggested, unconfirmed",
        V::HumanConfirmed => "human confirmed",
        V::Disputed => "disputed",
        V::Refuted => "refuted",
    }
}

/// Say which redesignation links the report saw and did not follow.
///
/// Silence here would put the reader back where #165 found them: a path named
/// by redesignation links, answered as though no link existed.
fn print_unfollowed(unfollowed: &[inspect::UnfollowedRedesignation]) {
    if unfollowed.is_empty() {
        return;
    }
    let no_window = unfollowed
        .iter()
        .all(|u| u.reason == inspect::NotFollowed::NoWindow);
    if no_window {
        println!(
            "\n{} redesignation link(s) name this path. Give --from and --to to resolve them.",
            unfollowed.len()
        );
        return;
    }

    println!("\nRedesignations not followed ({}):", unfollowed.len());
    for u in unfollowed {
        let why = match u.reason {
            inspect::NotFollowed::NoWindow => "no expression pair was given",
            inspect::NotFollowed::Refuted => "refuted, so it was checked and found wrong",
            // Who refuted it and when is on the line beneath, which prints the
            // winning review of every link it names.
            inspect::NotFollowed::ReviewRefuted => "a reviewer refuted it",
        };
        println!("  {} -> {} — {why}", u.link.from_path, u.link.to_path);
        println!("      {}", stated_by(&u.link));
    }
}

/// Where the provision sits on each side, using the same indices as `--json`.
///
/// A provision that shares its path with another is only addressable by
/// position, so the position is printed rather than left to the reader to
/// count off the list.
fn positions(p: &inspect::ProvisionAtPath) -> String {
    match (p.from_position, p.to_position) {
        (Some(from), Some(to)) => format!("from position {from}, to position {to}"),
        (Some(from), None) => format!("was at position {from}"),
        (None, Some(to)) => format!("now at position {to}"),
        (None, None) => "no position".to_string(),
    }
}

/// How many characters of one field a person is shown before it is cut.
///
/// A field is one node's text, and one node can hold a whole section: a real
/// search hit carried 10,228 characters (#235). Wide enough for a sentence of
/// the law, short enough that a screenful of fields is still a screenful.
const FIELD_WIDTH: usize = 300;

/// The words of a provision the window holds at one end only, and which way
/// they went.
///
/// A field diff has nothing to say about a provision with no other end, so
/// these words are the evidence a reviewer reads instead (#259). The label says
/// plainly whether they were added or removed, because the same words mean the
/// opposite in the two cases.
///
/// Each field is named by its path from `provision` down, so the reader can see
/// where in the provision the words sit.
///
/// A screenful, and no more: at most [`DEFAULT_LIMIT`] fields, each cut at
/// [`FIELD_WIDTH`] characters with the cut marked `…`. What was left out is said,
/// and `rest` tells the reader where to read all of it. A reader that stopped
/// in silence would read as the whole provision.
pub fn print_words(
    change: &str,
    provision: &str,
    words: &[inspect::ProvisionField],
    indent: &str,
    rest: &str,
) {
    if words.is_empty() {
        println!("{indent}The words {change}: none. The provision holds no text.");
        return;
    }
    // The provision's parent is cut off, so every line starts at the
    // provision itself.
    let parent = provision.rsplit_once('/').map_or("", |(parent, _)| parent);
    println!("{indent}The words {change} ({}):", fields(words.len()));
    for word in words.iter().take(DEFAULT_LIMIT) {
        let at = word
            .path
            .strip_prefix(parent)
            .map_or(word.path.as_str(), |below| below.trim_start_matches('/'));
        println!("{indent}  {at} [{}] {}", word.field, cut(&word.text));
    }
    let dropped = words.len().saturating_sub(DEFAULT_LIMIT);
    if dropped > 0 {
        let noun = if dropped == 1 { "field" } else { "fields" };
        println!("{indent}  … {dropped} more {noun} not shown.");
    }
    let any_cut = words
        .iter()
        .take(DEFAULT_LIMIT)
        .any(|word| word.text.chars().count() > FIELD_WIDTH);
    if dropped > 0 || any_cut {
        println!("{indent}  {rest}");
    }
}

/// The first [`FIELD_WIDTH`] characters of a field, with `…` where it was cut.
///
/// Counted in characters and not bytes. The corpus holds `§`, and a byte slice
/// through a character panics.
fn cut(text: &str) -> String {
    if text.chars().count() <= FIELD_WIDTH {
        return text.to_string();
    }
    let mut shown: String = text.chars().take(FIELD_WIDTH).collect();
    shown.push('…');
    shown
}

fn fields(n: usize) -> String {
    if n == 1 {
        "1 field".to_string()
    } else {
        format!("{n} fields")
    }
}
