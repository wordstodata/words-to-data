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
//!    list of nine is not.
//! 3. **What the dataset can say about the provision since.** The printings it
//!    holds, whether the provision changed between them, *and* the period between
//!    the opinion and the first printing held, which nothing in the file covers.
//!
//! `--chain` carries part three further, through
//! `legislature.amended_by` to the amendment, the bill, its sponsor and the roll
//! call. That needs a dataset with legislative material in it; one without is
//! told so rather than answered with an empty list.

use std::collections::BTreeSet;

use clap::Args as ClapArgs;
use words_to_data::citation::resolve::{Resolution, SectionPaths, resolve};
use words_to_data::citation::usc;
use words_to_data::dataset::{Coverage, ExpressionId, WorkId};
use words_to_data::document::NodeType;
use words_to_data::judicial::reliance::{self, ChangeWindow, Citing, CitingCase};
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
}

pub fn run(args: Args) {
    let opened = crate::fail::or_exit(crate::load::open(&args.dataset), "Error opening dataset");
    with_dataset!(opened, dataset => report(&dataset, &args));
}

fn report<S: Storage>(dataset: &S, args: &Args) {
    let paths = match (&args.path, &args.cites) {
        (Some(path), _) => vec![path.clone()],
        (None, Some(citation)) => resolved_paths(dataset, citation),
        (None, None) => {
            eprintln!("Give either --cites \"26 U.S.C. § 174\" or --path <structural path>.");
            std::process::exit(1);
        }
    };

    for path in &paths {
        println!("\n=== {path}");
        let cases = crate::fail::or_exit(
            reliance::cases_citing(dataset, path),
            "Error reading citations",
        );
        report_cases(&cases);
        report_silent_opinions(dataset, &cases);
        if args.chain {
            report_chain(dataset, &cases);
        }
    }
}

/// Where a citation resolves in this dataset, reported honestly.
///
/// Exits without an answer where the dataset does not carry the title. That is
/// not a failure of the query: it is the query's answer, and pretending to run it
/// anyway would print an empty list that reads as "no case cites this".
fn resolved_paths<S: Storage>(dataset: &S, text: &str) -> Vec<String> {
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
    println!("{} names {}", citation.text, work);

    match scope.covers(work.as_str()) {
        Coverage::OutOfScope => {
            println!(
                "  out of scope: this dataset does not carry {work}, so it can say \
                 nothing about the section. This is not \"not found\"."
            );
            return Vec::new();
        }
        Coverage::Gap => {
            println!(
                "  gap: this dataset was declared to carry {work} and does not. \
                 That is a fault in the build, not a statement about the law."
            );
            return Vec::new();
        }
        Coverage::InScope => {}
    }

    let mut index = SectionPaths::new();
    index_work(dataset, &work, &mut index);

    let mut paths = Vec::new();
    for section in resolve(citation, &scope, &index) {
        match section.resolution {
            Resolution::Provision { paths: found } => {
                println!(
                    "  {} resolves to {} provision(s)",
                    section.uslm_id,
                    found.len()
                );
                paths.extend(found);
            }
            Resolution::Absent => println!(
                "  absent: this dataset carries {work} and it holds no section {}. \
                 That is a statement about the law as this dataset records it.",
                section.section
            ),
            Resolution::OutOfScope => println!("  out of scope: {}", section.uslm_id),
            Resolution::Gap => println!("  gap: {}", section.uslm_id),
        }
    }
    paths
}

/// Index the latest printing of one work, so a citation can be turned into a path.
fn index_work<S: Storage>(dataset: &S, work: &WorkId, index: &mut SectionPaths) {
    let Ok(expressions) = dataset.expressions(work) else {
        return;
    };
    let Some(latest) = expressions.last() else {
        return;
    };
    let id = ExpressionId::new(work.clone(), latest.id.at.clone());
    if let Ok(Some(expression)) = dataset.get_expression(&id) {
        index.add_work(&expression.root);
    }
}

fn report_cases(cases: &[CitingCase]) {
    if cases.is_empty() {
        println!("No opinion in this dataset cites it.");
        return;
    }

    println!("\n{} opinion(s) cite it:\n", cases.len());
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
            report_since(&cited.since);
        }
        println!();
    }

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

fn report_since(since: &reliance::SincePublication) {
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
    for ChangeWindow {
        from,
        to,
        changed,
        changed_paths,
    } in &since.windows
    {
        match changed {
            true => println!(
                "      {from} → {to}: CHANGED, at {} path(s){}",
                changed_paths.len(),
                first_paths(changed_paths),
            ),
            false => println!("      {from} → {to}: unchanged"),
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

/// The opinions this dataset holds that do **not** cite the provision.
///
/// "Which of the ten cite it" is answered by both halves. A list of the nine that
/// do, with the tenth left out, cannot be told from a list of nine opinions.
///
/// A judicial work is found by the first segment of its structural path, which is
/// the class (`docs/adr/0006-a-document-node-is-class-neutral.md`). Reading every
/// node's type instead would mean loading every tree in the dataset, and a title
/// of the U.S. Code is tens of thousands of nodes.
fn report_silent_opinions<S: Storage>(dataset: &S, cases: &[CitingCase]) {
    let citing: BTreeSet<&str> = cases
        .iter()
        .filter_map(|case| match &case.citing {
            Citing::Held { expression, .. } => Some(expression.work.as_str()),
            Citing::NotHeld { .. } => None,
        })
        .collect();

    let Ok(works) = dataset.works() else {
        return;
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

    let silent: Vec<&WorkId> = judicial
        .iter()
        .filter(|work| !citing.contains(work.as_str()))
        .collect();

    println!(
        "This dataset holds {} opinion(s). {} of them do not cite it:",
        judicial.len(),
        silent.len(),
    );
    for work in &silent {
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
