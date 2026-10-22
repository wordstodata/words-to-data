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
    Resolution, SectionPaths, Subdivision, resolve, resolve_subdivision,
};
use words_to_data::citation::usc;
use words_to_data::dataset::{Coverage, Expression, ExpressionId, WorkId};
use words_to_data::document::{DocumentNode, NodeType};
use words_to_data::judicial::reliance::{
    self, ChangeWindow, CitedProvision, Citing, CitingCase, Relation,
};
use words_to_data::link::{LinkKind, Target, amendment_reference_parts};
use words_to_data::method::Method;
use words_to_data::storage::Storage;

use crate::load::with_dataset;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite)
    pub dataset: String,

    /// The citation to ask about, as a court would write it: `26 U.S.C. § 174`
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
        (Some(path), _) => vec![path.clone()],
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
        let (judicial, silent) = silent_opinions(dataset, &cases);

        if args.json {
            answers.push(serde_json::json!({
                "path": path,
                "held": held,
                "cases": cases,
                "opinions_held": judicial,
                "not_citing": silent,
                "notes": [ONE_LINK_PER_SECTION],
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
        let (within, holding): (Vec<CitingCase>, Vec<CitingCase>) =
            cases.iter().cloned().partition(cites_the_path_itself);
        report_cases(&within, holding.is_empty());
        report_holding(&holding);
        report_silent_opinions(judicial, &silent);
        if !cases.is_empty() {
            println!("\n{ONE_LINK_PER_SECTION}");
        }
        if args.chain {
            report_chain(dataset, &cases);
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

/// Where a citation resolves in this dataset, reported honestly.
///
/// Exits without an answer where the dataset does not carry the title. That is
/// not a failure of the query: it is the query's answer, and pretending to run it
/// anyway would print an empty list that reads as "no case cites this".
fn resolved_paths<S: Storage>(dataset: &S, text: &str, json: bool) -> Vec<String> {
    let (found, report) = usc::find_with_report(text);
    report.print_to_stderr();

    let Some(citation) = found.first() else {
        eprintln!(
            "`{text}` is not a U.S. Code citation this build reads. Write it as \
             a title, a reporter and a section, such as `26 U.S.C. § 174`."
        );
        std::process::exit(1);
    };

    let scope = crate::fail::or_exit(
        words_to_data::dataset::Scope::derive(dataset),
        "Error reading the dataset's scope",
    );
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
    for section in resolve(citation, &scope, &index) {
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
                    paths.push(subdivision_path(held, &section.section, path, json));
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
fn subdivision_path(
    section: Option<&DocumentNode>,
    written: &str,
    path: String,
    json: bool,
) -> String {
    let Some(section) = section else {
        return path;
    };
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

/// The known limit of a citation link, said wherever one is reported.
const ONE_LINK_PER_SECTION: &str = "Note: a link is identified by the opinion and the \
section, so it keeps only the last citation of one section in one opinion. An opinion \
that cites (a) and later (e) of one section shows only (e) here.";

/// Whether any of this opinion's citations is to the path asked about, or a
/// provision under it, and not only to a provision that holds the path.
fn cites_the_path_itself(case: &CitingCase) -> bool {
    case.cites
        .iter()
        .any(|cited| cited.relation == Relation::Within)
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

    // The tally the question actually asks for.
    let changed: Vec<&CitingCase> = cases
        .iter()
        .filter(|case| {
            case.cites
                .iter()
                .any(|cited| cited.since.changed_in_a_covered_window())
        })
        .collect();
    println!(
        "{} of {} citing opinion(s) cite a provision that changed in a window \
         this dataset covers.",
        changed.len(),
        cases.len()
    );
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
                "    cites {} — link is {:?}, matched {:?}",
                cited.path,
                cited.verification,
                cited.citation_text.as_deref().unwrap_or(""),
            );
            report_subdivision(cited);
            report_stub(cited);
            report_since(cited);
        }
        println!();
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
        println!(
            "      this dataset holds fewer than two printings after the opinion, \
             so it cannot say whether the provision changed"
        );
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
                "      {from} → {to}: CHANGED {at}; {} path(s) changed{}",
                changed_paths.len(),
                first_paths(changed_paths),
            ),
            false => println!("      {from} → {to}: unchanged {at}"),
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

/// Where a window was read, said so that a reader can tell the subdivision the
/// opinion wrote from the whole section.
fn window_place(cited: &CitedProvision, window: &ChangeWindow) -> String {
    match &cited.subdivision {
        Subdivision::Section => format!("at {}", window.at),
        _ if window.at != cited.path => format!("at {}", window.at),
        _ => format!(
            "at {} (the window covers the whole section, because the subdivision \
             written does not resolve in these printings)",
            window.at
        ),
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

/// How many opinions this dataset holds, and those that do **not** cite the
/// provision.
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
        "This dataset holds {held} opinion(s). {} of them do not cite it:",
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
fn report_chain<S: Storage>(dataset: &S, cases: &[CitingCase]) {
    let changed: BTreeSet<&str> = cases
        .iter()
        .flat_map(|case| &case.cites)
        .flat_map(|cited| &cited.since.windows)
        .flat_map(|window| &window.changed_paths)
        .map(String::as_str)
        .collect();

    println!("\n--- the chain out of the change");
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
