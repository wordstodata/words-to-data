//! `words_to_data cases-citing` — which cases cite a provision, and has it moved
//! under them since.
//!
//! This is the query #53 exists for:
//!
//! > which of the ten opinions cite 26 USC 174, and did that provision change
//! > after the opinion was published?
//!
//! It answers in three parts, and the third is the one that is easy to get
//! quietly wrong.
//!
//! 1. **Where the citation resolves.** A title the dataset does not carry is
//!    reported out of scope, never "not found": a reader who takes absence for
//!    "no such authority" has been misled by the tool.
//! 2. **Which held opinions cite it**, with the verification state of each link
//!    and the text the rule matched. Opinions that do not cite it are listed as
//!    well, because "these nine do and this one does not" is the answer, and a
//!    list of nine is not. Asked about a path below the section, an opinion
//!    that cites the section alone, or a subdivision above the path, is listed
//!    in a group of its own and never as "does not cite" (#302).
//! 3. **What the dataset can say about the provision since.** The printings it
//!    holds, whether the provision changed between them, *and* the period between
//!    the opinion and the first printing held, which nothing in the file covers.
//!    A change is read at the subdivision the opinion wrote when both printings
//!    hold it, and at the whole section when they do not, and the line says
//!    which.
//!
//! `--chain` carries part three further, through
//! `legislature.amended_by` to the amendment, the bill, its sponsor and the roll
//! call. That needs a dataset with legislative material in it; one without is
//! told so rather than answered with an empty list.

use std::collections::BTreeSet;

use clap::Args as ClapArgs;
use words_to_data::citation::resolve::{
    Resolution, SectionPaths, Subdivision, reach_range, resolve, resolve_subdivision,
};
use words_to_data::citation::usc;
use words_to_data::dataset::{Coverage, Expression, ExpressionId, WorkId};
use words_to_data::document::{DocumentNode, NodeType, TextContentField};
use words_to_data::judicial::reliance::{
    self, ChangeWindow, ChangedAbove, CitedProvision, Citing, CitingCase, Relation,
};
use words_to_data::link::{LinkKind, Target, amendment_reference_parts};
use words_to_data::method::Method;
use words_to_data::query::{Locator, PathMatch};
use words_to_data::storage::Storage;

use crate::load::with_dataset;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite)
    pub dataset: String,

    /// The citation to ask about: `26 U.S.C. § 174`, `26 U.S.C. 174` or
    /// `26 USC 174`. The section marker is optional
    #[arg(long, conflicts_with = "path")]
    pub cites: Option<String>,

    /// The structural path to ask about, when it is already known
    #[arg(long, conflicts_with = "cites")]
    pub path: Option<String>,

    /// Follow each change on to the amendment, the bill, its sponsor and the vote
    #[arg(long)]
    pub chain: bool,

    /// Emit JSON instead of human-readable text: one entry per path asked about
    #[arg(long, conflicts_with = "chain")]
    pub json: bool,
}

pub fn run(args: Args) {
    let opened = crate::fail::or_exit(crate::load::open(&args.dataset), "Error opening dataset");
    with_dataset!(opened, dataset => report(&dataset, &args));
}

fn report<S: Storage>(dataset: &S, args: &Args) {
    let paths = match (&args.path, &args.cites) {
        (Some(path), _) => vec![crate::held::path_argument(dataset, path)],
        (None, Some(citation)) => resolved_paths(dataset, citation, args.json),
        (None, None) => {
            eprintln!("Give either --cites \"26 U.S.C. § 174\" or --path <structural path>.");
            std::process::exit(1);
        }
    };

    let mut answers = Vec::new();
    for path in &paths {
        let held = dataset.has_node(path).unwrap_or(true);
        let cases = crate::fail::or_exit(
            reliance::cases_citing(dataset, path),
            "Error reading citations",
        );
        // A path typed with `--path` in a work the dataset carries, that no
        // printing holds and no opinion cites, is not held: the run refuses
        // it, as `diff` does (#392, #425). A work the dataset does not carry
        // is answered as out of scope, as `--cites` answers it.
        if !held && cases.is_empty() && args.path.is_some() && in_scope(dataset, path) {
            crate::held::refuse_unless_path_held_in(
                dataset,
                &Locator::new().at_path(path, PathMatch::Subtree),
                false,
                &crate::held::scope_of(None, None),
            );
        }
        let (judicial, silent) = silent_opinions(dataset, &cases);

        if args.json {
            answers.push(serde_json::json!({
                "path": path,
                "held": held,
                "cases": cases,
                "cases_held": judicial,
                "not_citing": silent,
                "notes": [ONE_LINK_PER_PASSAGE],
            }));
            continue;
        }

        println!("\n=== {path}");
        if !held {
            println!(
                "No printing this dataset holds has this path. An opinion can still \
                 cite the section that holds it, and those are listed below."
            );
        }
        // A writing can cite the path itself in one passage and the section
        // that holds it in another. Each citation is told in its own group.
        let within = with_relation(&cases, Relation::Within);
        let holding = with_relation(&cases, Relation::Holds);
        report_cases(&within, holding.is_empty());
        report_holding(&holding);
        report_silent_opinions(judicial, &silent);
        if !cases.is_empty() {
            println!("\n{ONE_LINK_PER_PASSAGE}");
        }
        if args.chain {
            report_chain(dataset, path, &cases);
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&answers).expect("the answer should serialize")
        );
    }
}

/// Print a line about how a citation was resolved: to stdout for a person, and
/// to stderr under `--json`, so stdout stays one JSON value.
fn say(json: bool, line: &str) {
    match json {
        true => eprintln!("{line}"),
        false => println!("{line}"),
    }
}

/// Where each citation in `text` resolves in this dataset, reported honestly.
///
/// Every citation the text holds is answered: "§ 174 and § 162" answered § 174
/// alone, and said nothing of § 162 (#426).
fn resolved_paths<S: Storage>(dataset: &S, text: &str, json: bool) -> Vec<String> {
    let (found, report) = usc::find_typed(text);
    report.print_to_stderr();

    if found.is_empty() {
        eprintln!(
            "`{text}` is not a U.S. Code citation this build reads. Write it as \
             a title, a reporter and a section, such as `26 U.S.C. § 174`."
        );
        std::process::exit(1);
    }

    let scope = crate::fail::or_exit(
        words_to_data::dataset::Scope::derive(dataset),
        "Error reading the dataset's scope",
    );
    found
        .iter()
        .flat_map(|citation| citation_paths(dataset, &scope, citation, json))
        .collect()
}

/// Where one citation resolves in this dataset, reported honestly.
///
/// Gives no path where the dataset does not carry the title. That is not a
/// failure of the query: it is the query's answer, and pretending to run it
/// anyway would print an empty list that reads as "no case cites this".
fn citation_paths<S: Storage>(
    dataset: &S,
    scope: &words_to_data::dataset::Scope,
    citation: &usc::UscCitation,
    json: bool,
) -> Vec<String> {
    let work = citation.work();
    say(json, &format!("{} names {}", citation.text, work));

    match scope.covers(work.as_str()) {
        Coverage::OutOfScope => {
            say(
                json,
                &format!(
                    "  out of scope: this dataset does not carry {work}, so it can say \
                     nothing about the section. This is not \"not found\"."
                ),
            );
            return Vec::new();
        }
        Coverage::Gap => {
            say(
                json,
                &format!(
                    "  gap: this dataset was declared to carry {work} and does not. \
                     That is a fault in the build, not a statement about the law."
                ),
            );
            return Vec::new();
        }
        Coverage::InScope => {}
    }

    let mut index = SectionPaths::new();
    let latest = latest_printing(dataset, &work);
    if let Some(latest) = &latest {
        index.add_work(&latest.root);
    }

    let mut paths = Vec::new();
    for section in resolve(citation, scope, &index) {
        match section.resolution {
            Resolution::Provision { paths: found } => {
                say(
                    json,
                    &format!(
                        "  {} resolves to {} provision(s)",
                        section.uslm_id,
                        found.len()
                    ),
                );
                for path in found {
                    let held = latest.as_ref().and_then(|latest| latest.root.find(&path));
                    paths.extend(subdivision_paths(held, &section.section, path, json));
                }
            }
            Resolution::Absent => say(
                json,
                &format!(
                    "  absent: this dataset carries {work} and it holds no section {}. \
                     That is a statement about the law as this dataset records it.",
                    section.section
                ),
            ),
            Resolution::OutOfScope => say(json, &format!("  out of scope: {}", section.uslm_id)),
            Resolution::Gap => say(json, &format!("  gap: {}", section.uslm_id)),
        }
    }
    paths
}

/// The latest printing of one work, which a citation is resolved in.
fn latest_printing<S: Storage>(dataset: &S, work: &WorkId) -> Option<Expression> {
    let expressions = dataset.expressions(work).ok()?;
    let latest = expressions.last()?;
    let id = ExpressionId::new(work.clone(), latest.id.at.clone());
    dataset.get_expression(&id).ok().flatten()
}

/// The path a citation's subdivision leads to below the section at `path`, or
/// the section, with a line that says which part did not resolve.
///
/// The part is named and never dropped: `26 U.S.C. § 174(e)` asked of a
/// printing with no (e) is answered about the whole section, and the reader is
/// told so.
///
/// A range of subdivisions, `994(a)(1)-(3)`, is a path for each member
/// ([`reach_range`], #366). A member or a range that does not resolve is named,
/// and answered about the provision above it.
fn subdivision_paths(
    section: Option<&DocumentNode>,
    written: &str,
    path: String,
    json: bool,
) -> Vec<String> {
    let Some(section) = section else {
        return vec![path];
    };
    let Some(range) = usc::subdivision_range(written) else {
        return vec![subdivision_path(section, written, path, json)];
    };
    let mut paths: Vec<String> = Vec::new();
    for reached in reach_range(section, &range) {
        match &reached.unresolved {
            Some(unresolved) => say(
                json,
                &format!(
                    "  {unresolved} of {written} does not resolve in the latest printing \
                 held, so this answers about {}",
                    reached.path
                ),
            ),
            None => say(json, &format!("  {written} reaches {}", reached.path)),
        }
        if !paths.contains(&reached.path) {
            paths.push(reached.path);
        }
    }
    paths
}

/// The path the subdivision `written` leads to below `section`, one written
/// with no range ([`subdivision_paths`]).
fn subdivision_path(section: &DocumentNode, written: &str, path: String, json: bool) -> String {
    match resolve_subdivision(section, written) {
        Subdivision::Section => path,
        Subdivision::Resolved {
            resolved_path: below,
        } => {
            say(json, &format!("  {written} resolves to {below}"));
            below
        }
        Subdivision::Unresolved { unresolved } => {
            say(
                json,
                &format!(
                    "  {unresolved} of {written} does not resolve in the latest printing \
                 held, so this answers about the whole section"
                ),
            );
            path
        }
    }
}

/// How a citation link is made, said wherever one is reported (#316).
const ONE_LINK_PER_PASSAGE: &str = "Note: a citation link starts at the paragraph or \
footnote that holds the citation and ends at the subdivision the court wrote. Each \
passage that cites a provision is one link, and its payload holds every place in \
the passage where the court wrote it.";

/// Each case with only its citations in `relation` to the path asked about,
/// and only the cases that have one.
fn with_relation(cases: &[CitingCase], relation: Relation) -> Vec<CitingCase> {
    cases
        .iter()
        .filter_map(|case| {
            let cites: Vec<CitedProvision> = case
                .cites
                .iter()
                .filter(|cited| cited.relation == relation)
                .cloned()
                .collect();
            (!cites.is_empty()).then(|| CitingCase {
                citing: case.citing.clone(),
                cites,
            })
        })
        .collect()
}

fn report_cases(cases: &[CitingCase], none_holds_it: bool) {
    if cases.is_empty() {
        match none_holds_it {
            true => println!("No opinion in this dataset cites it."),
            false => println!(
                "No opinion in this dataset cites this path itself, or a provision under it."
            ),
        }
        return;
    }

    println!("\n{} opinion(s) cite it:\n", cases.len());
    report_each(cases);

    // The tally the question actually asks for. A change that cannot be put
    // after the opinion is counted apart, never as a change after it (#340).
    let count = |kind: Tally| cases.iter().filter(|case| tally(case) == kind).count();
    println!(
        "{} of {} citing opinion(s) cite a provision that changed in a window \
         this dataset covers that starts on or after the opinion.",
        count(Tally::ChangedAfter),
        cases.len()
    );
    let unplaced = count(Tally::ChangedInAWindowThatStartsBefore);
    if unplaced > 0 {
        println!(
            "{unplaced} more cite a provision that changed only in a window that \
             starts before the opinion, so this dataset cannot say whether that change \
             came before or after the decision."
        );
    }
    let above = count(Tally::ChangedAbove);
    if above > 0 {
        println!(
            "{above} more cite a subdivision whose own words did not change, under \
             a provision above it that changed in a window this dataset covers."
        );
    }
}

/// How one citing opinion counts in the tally, from the strongest answer down.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tally {
    /// A cited provision changed in a window that starts on or after the
    /// opinion.
    ChangedAfter,
    /// A cited provision changed only in a window that starts before the
    /// opinion and ends after it, so the change can be from either side.
    ChangedInAWindowThatStartsBefore,
    /// No cited provision changed, and a provision above one of them did.
    ChangedAbove,
    Unchanged,
}

fn tally(case: &CitingCase) -> Tally {
    let windows = || case.cites.iter().flat_map(|cited| &cited.since.windows);
    if windows().any(|window| window.changed && !window.starts_before_opinion) {
        Tally::ChangedAfter
    } else if windows().any(|window| window.changed) {
        Tally::ChangedInAWindowThatStartsBefore
    } else if windows().any(|window| !window.changed_above.is_empty()) {
        Tally::ChangedAbove
    } else {
        Tally::Unchanged
    }
}

/// The opinions that cite a provision above the path asked about: the section
/// alone, a subdivision that holds the path, or a subdivision the printing does
/// not hold. They are not "does not cite", and they are not "cites it".
fn report_holding(cases: &[CitingCase]) {
    if cases.is_empty() {
        return;
    }
    println!(
        "\n{} opinion(s) cite a provision that holds this path, and not this path \
         itself. Each line says what the opinion wrote:\n",
        cases.len()
    );
    report_each(cases);
}

fn report_each(cases: &[CitingCase]) {
    for case in cases {
        println!("  {}", case.citing.display());
        match &case.citing {
            Citing::Held {
                text_source,
                text_method,
                text_verification,
                ..
            } => println!(
                "    opinion text: {} / {} / {}",
                text_source.as_deref().unwrap_or("unrecorded"),
                text_method
                    .as_ref()
                    .map_or_else(|| "unrecorded".to_string(), Method::to_string),
                match text_verification {
                    Some(state) => format!("{state:?}"),
                    None => "unrecorded".to_string(),
                },
            ),
            Citing::NotHeld { reference, .. } => println!(
                "    opinion text: not in this dataset ({reference}), so nothing \
                 can be said about it"
            ),
        }

        for cited in &case.cites {
            println!(
                "    cites {}, from {} passage(s):",
                cited.path,
                cited.places.len()
            );
            for place in &cited.places {
                // The review's verdict where the link has one, and who gave
                // it. The stored state alone read as "nobody reviewed it"
                // (#415).
                let trust = match &place.review {
                    None => format!("{:?}", place.verification),
                    Some(reported) => format!(
                        "{:?}, {} by {} on {}",
                        place.status,
                        reported.review.verdict,
                        reported.review.reviewer,
                        reported.review.at.date()
                    ),
                };
                println!(
                    "      {} — link {} is {trust}, matched {:?}{}",
                    place.passage,
                    place.link,
                    place.citation_text.as_deref().unwrap_or(""),
                    match place.unresolved.as_slice() {
                        [] => String::new(),
                        parts => format!(
                            "; {} does not resolve in the printing the link was made against",
                            parts.join(", ")
                        ),
                    },
                );
            }
            report_dates(cited);
            report_subdivision(cited);
            report_stub(cited);
            report_since(cited);
        }
        println!();
    }
}

/// When the court read the law, and which printing the citation was resolved
/// in (#317). Each different statement once: the places of one writing carry
/// one decision date.
fn report_dates(cited: &CitedProvision) {
    let title = cited
        .path
        .split('/')
        .nth(1)
        .and_then(|segment| segment.strip_prefix("title_"))
        .unwrap_or("?");
    let mut said: BTreeSet<String> = BTreeSet::new();
    for place in &cited.places {
        let line = match (&place.decided, &place.resolved_in) {
            (Some(decided), Some(printed)) => match place.resolved_after_decision {
                Some(true) => format!(
                    "      decided {decided}. The link was resolved in the printing of \
                     {printed}, the earliest held: this dataset holds no printing of the \
                     law as it stood on {decided}, so that law is out of scope."
                ),
                _ => format!(
                    "      decided {decided}. The link was resolved in the printing of \
                     {printed}, the latest held on or before that date."
                ),
            },
            _ => continue,
        };
        if said.insert(line.clone()) {
            println!("{line}");
        }
        if let Some(year) = &place.time_basis {
            let line = match place.printings_of_time_basis.as_slice() {
                [] => format!(
                    "      the opinion states it applies title {title} as in effect for \
                     {year}. This dataset holds no printing of title {title} from {year}, \
                     so that law is out of scope."
                ),
                dates => format!(
                    "      the opinion states it applies title {title} as in effect for \
                     {year}. This dataset holds the printing(s) of {} from that year.",
                    dates.join(", ")
                ),
            };
            if said.insert(line.clone()) {
                println!("{line}");
            }
        }
    }
}

/// What the opinion wrote below the section, and where it leads.
fn report_subdivision(cited: &CitedProvision) {
    match &cited.subdivision {
        Subdivision::Section => println!("      wrote the section alone"),
        Subdivision::Resolved { resolved_path } => {
            println!(
                "      wrote a subdivision: it resolves to {resolved_path} in the latest printing held"
            )
        }
        Subdivision::Unresolved { unresolved } => println!(
            "      wrote a subdivision: {unresolved} does not resolve in the latest \
             printing held, so only the section is known"
        ),
    }
}

/// Whether the cited section is a stub in the latest printing held (#310).
/// Said only when it is: a section of law needs no line.
fn report_stub(cited: &CitedProvision) {
    if let Some(stub) = &cited.stub_heading {
        println!(
            "      the cited section is now a stub: {stub}. That is its heading in the \
             latest printing held, which holds no law at that number."
        );
    }
}

fn report_since(cited: &CitedProvision) {
    let since = &cited.since;
    match since.coverage {
        Coverage::OutOfScope => {
            println!("      out of scope: this dataset does not carry the cited material");
            return;
        }
        Coverage::Gap => {
            println!("      gap: the dataset was declared to carry this and does not");
            return;
        }
        Coverage::InScope => {}
    }

    if since.windows.is_empty() {
        match since.printings_after {
            0 => println!(
                "      this dataset holds no printing of the cited work after the \
                 opinion, so it cannot say whether the provision changed since"
            ),
            _ => println!(
                "      this dataset holds one printing of the cited work after the \
                 opinion, and no printing before it to compare it with, so it \
                 cannot say whether the provision changed since"
            ),
        }
    }
    for window in &since.windows {
        let ChangeWindow {
            from,
            to,
            changed,
            changed_paths,
            ..
        } = window;
        let at = window_place(cited, window);
        match changed {
            true => println!(
                "      {from} → {to}: CHANGED {at}{}; {} path(s) changed{}",
                match window.starts_before_opinion {
                    true => STARTS_BEFORE_THE_OPINION,
                    false => "",
                },
                changed_paths.len(),
                first_paths(changed_paths),
            ),
            false => println!(
                "      {from} → {to}: unchanged {at}{}",
                changed_above(&window.changed_above)
            ),
        }
    }

    if let Some(uncovered) = &since.uncovered {
        println!(
            "      {} → {}: OUT OF SCOPE. This dataset holds no printing of the \
             cited work in that period, so it cannot say whether the provision \
             changed in it. It is not a statement that nothing changed.",
            uncovered.from, uncovered.to,
        );
    }
}

/// Said on a CHANGED line whose window starts before the opinion and ends after
/// it. The change is not a change after the decision until a printing says so
/// (#340).
const STARTS_BEFORE_THE_OPINION: &str = ", in a window that starts before the opinion: \
this dataset cannot say whether the change came before or after the decision";

/// Where a window was read, said so that a reader can tell the subdivision the
/// opinion wrote from the whole section.
fn window_place(cited: &CitedProvision, window: &ChangeWindow) -> String {
    let at_a_section = window
        .at
        .rsplit('/')
        .next()
        .is_some_and(|last| last.starts_with("section_"));
    match &cited.subdivision {
        Subdivision::Section => format!("at {}", window.at),
        _ if !at_a_section => format!("at {}", window.at),
        _ => format!(
            "at {} (the window covers the whole section, because the subdivision \
             written does not resolve in these printings)",
            window.at
        ),
    }
}

/// The provisions above the cited subdivision whose own words changed, said on
/// the line that calls the subdivision unchanged. Its words can stay the same
/// while a chapeau above them gives them another meaning (#340).
fn changed_above(above: &[ChangedAbove]) -> String {
    above
        .iter()
        .map(|provision| {
            let fields: Vec<&str> = provision.fields.iter().map(field_name).collect();
            format!(
                "; the {} of {} changed",
                fields.join(" and "),
                provision.path
            )
        })
        .collect()
}

fn field_name(field: &TextContentField) -> &'static str {
    match field {
        TextContentField::Heading => "heading",
        TextContentField::Chapeau => "chapeau",
        TextContentField::Proviso => "proviso",
        TextContentField::Content => "content",
        TextContentField::Continuation => "continuation",
        TextContentField::Footnotes => "footnotes",
    }
}

/// The first few changed paths, so the line stays a line.
fn first_paths(paths: &[String]) -> String {
    const SHOWN: usize = 3;
    if paths.is_empty() {
        return String::new();
    }
    let named: Vec<&str> = paths.iter().take(SHOWN).map(String::as_str).collect();
    let rest = paths.len().saturating_sub(SHOWN);
    match rest {
        0 => format!(": {}", named.join(", ")),
        more => format!(": {}, and {more} more", named.join(", ")),
    }
}

/// How many cases this dataset holds, and those with no writing that cites the
/// provision. A case is a work, `judicial/case_<cluster>`, so a case counts as
/// citing when any of its writings does (#313).
///
/// "Which of the ten cite it" is answered by both halves. A list of the nine that
/// do, with the tenth left out, cannot be told from a list of nine opinions.
///
/// A judicial work is found by the first segment of its structural path, which is
/// the class (`docs/adr/0006-a-document-node-is-class-neutral.md`). Reading every
/// node's type instead would mean loading every tree in the dataset, and a title
/// of the U.S. Code is tens of thousands of nodes.
fn silent_opinions<S: Storage>(dataset: &S, cases: &[CitingCase]) -> (usize, Vec<WorkId>) {
    let citing: BTreeSet<&str> = cases
        .iter()
        .filter_map(|case| match &case.citing {
            Citing::Held { expression, .. } => Some(expression.work.as_str()),
            Citing::NotHeld { .. } => None,
        })
        .collect();

    let Ok(works) = dataset.works() else {
        return (0, Vec::new());
    };
    let judicial: Vec<WorkId> = works
        .into_iter()
        .filter(|work| {
            work.as_str()
                .split('/')
                .next()
                .is_some_and(|class| class == NodeType::JUDICIAL)
        })
        .collect();

    let held = judicial.len();
    let silent = judicial
        .into_iter()
        .filter(|work| !citing.contains(work.as_str()))
        .collect();
    (held, silent)
}

fn report_silent_opinions(held: usize, silent: &[WorkId]) {
    println!(
        "This dataset holds {held} case(s). {} of them have no writing that cites it:",
        silent.len(),
    );
    for work in silent {
        println!("  {work}");
    }
}

/// Follow every change on to the amendment that caused it, and out to the vote.
///
/// The whole thesis in one walk: an opinion cites a provision, the provision
/// changed, a bill changed it, a member sponsored the bill and the House voted on
/// it. Each step is a link or a stored record, and a step the dataset cannot take
/// is named rather than skipped.
fn report_chain<S: Storage>(dataset: &S, path: &str, cases: &[CitingCase]) {
    let changed: BTreeSet<&str> = cases
        .iter()
        .flat_map(|case| &case.cites)
        .flat_map(|cited| &cited.since.windows)
        .flat_map(|window| &window.changed_paths)
        .map(String::as_str)
        .collect();

    println!("\n--- the chain out of the change");
    if cases.is_empty() {
        report_changes_held(dataset, path);
        return;
    }
    if changed.is_empty() {
        println!("Nothing changed in a covered window, so there is no chain to walk.");
        return;
    }

    let bills = amending_bills(dataset, &changed);
    if bills.is_empty() {
        println!(
            "{} provision(s) changed and no `legislature.amended_by` link names \
             what changed them. The change is recorded; the cause is not.",
            changed.len()
        );
        return;
    }

    // The run-time door (#127). A dataset of opinions alone holds no legislative
    // material, and "there is no legislature here" is a different answer from
    // "no bill matched", which is the difference a researcher needs.
    let Some(legislature) = dataset.legislature() else {
        println!(
            "{} bill(s) are named as the cause: {}.\n\
             This dataset holds no legislative material, so the sponsor and the \
             vote cannot be read from it. That is not the same as their being \
             absent.",
            bills.len(),
            bills.iter().cloned().collect::<Vec<_>>().join(", "),
        );
        return;
    };

    for bill_id in &bills {
        println!("\n{bill_id} amended the provision. Reading its record:");

        match legislature.get_sponsor_info(bill_id) {
            Ok(Some(sponsor)) => match legislature.get_member(&sponsor.sponsor) {
                Ok(Some(member)) => println!(
                    "  sponsor: {} ({}), {} cosponsor(s)",
                    member.name,
                    sponsor.sponsor,
                    sponsor.cosponsors.len()
                ),
                _ => println!(
                    "  sponsor: {} — the dataset holds no member record for them",
                    sponsor.sponsor
                ),
            },
            Ok(None) => println!("  sponsor: the dataset holds no sponsor record for this bill"),
            Err(error) => println!("  sponsor: {error}"),
        }

        match legislature.get_bill_votes(bill_id) {
            Ok(Some(votes)) if !votes.roll_calls.is_empty() => {
                for roll in &votes.roll_calls {
                    println!(
                        "  roll call {} on {}: {} — {} ({} yea, {} nay, {} member position(s) held)",
                        roll.roll_number,
                        roll.date,
                        roll.question,
                        roll.result,
                        roll.yea_count,
                        roll.nay_count,
                        roll.member_votes.len(),
                    );
                }
            }
            Ok(_) => println!("  roll call: the dataset holds no recorded vote for this bill"),
            Err(error) => println!("  roll call: {error}"),
        }
    }
}

/// What `--chain` can say when no opinion cites the path: there is no window
/// to walk, and the provision may still have changed in a window held (#340).
fn report_changes_held<S: Storage>(dataset: &S, path: &str) {
    println!("No opinion in this dataset cites it, so there is no window to walk.");
    let held = crate::fail::or_exit(
        reliance::changes_held(dataset, path),
        "Error reading the printings",
    );
    if held.coverage != Coverage::InScope {
        println!("This dataset does not carry the provision, so it cannot say whether it changed.");
        return;
    }
    let windows = &held.windows;
    if windows.is_empty() {
        println!(
            "This dataset holds fewer than two printings of the provision, so it \
             cannot say whether it changed."
        );
        return;
    }
    let changed: Vec<&ChangeWindow> = windows.iter().filter(|window| window.changed).collect();
    if changed.is_empty() {
        println!(
            "The provision did not change in any of the {} window(s) of printings \
             this dataset holds.",
            windows.len()
        );
        return;
    }
    println!("The provision changed in these windows of printings this dataset holds:");
    for window in changed {
        println!(
            "  {} → {}: CHANGED at {}; {} path(s) changed{}",
            window.from,
            window.to,
            window.at,
            window.changed_paths.len(),
            first_paths(&window.changed_paths),
        );
    }
}

/// The bills whose amendments are linked to any of these paths.
fn amending_bills<S: Storage>(dataset: &S, paths: &BTreeSet<&str>) -> BTreeSet<String> {
    let mut bills = BTreeSet::new();

    for path in paths {
        let Ok(links) = dataset.links_for_path(path) else {
            continue;
        };
        for link in links {
            if link.kind.0 != LinkKind::AMENDED_BY {
                continue;
            }
            if let Target::External { reference, .. } = &link.object
                && let Some((bill, _)) = amendment_reference_parts(reference)
            {
                bills.insert(bill.to_string());
            }
        }
    }

    bills
}

/// Whether the dataset carries the work that holds `path`.
fn in_scope<S: Storage>(dataset: &S, path: &str) -> bool {
    let scope = crate::fail::or_exit(
        words_to_data::dataset::Scope::derive(dataset),
        "Error reading the dataset's scope",
    );
    scope.covers(path) == Coverage::InScope
}
