//! `words_to_data contradictions` — the subjects a dataset holds more than one
//! link about.
//!
//! **A contradiction needs two makers.** A maker is a method at a version over a
//! window, and one maker's whole set of objects for a subject is **one answer**.
//! A subject with many links from a single maker therefore holds no
//! contradiction: a provision changed by three amendments of one bill is
//! ordinary law, and "Sections 1202(b)(2), 1202(g)(2)(A), and 1202(j)(1)(A) are
//! each amended by striking ..." is one instruction with three targets rather
//! than three competing claims.
//!
//! Two shapes, kept apart because they are different facts:
//!
//! * **Duplication** — two makers, and their answers match. One method run over
//!   two windows, placing one move twice.
//! * **Disagreement** — two makers, and their answers differ. Two answers that
//!   cannot both be right.
//!
//! Comparing links rather than makers is what this command did first, and it
//! reported 132 of the 530 annotated paths in the real corpus as disagreements.
//! None of them was one.
//!
//! **It reports, and it never resolves.** Decision 12 of #179 is settled: the
//! contradiction is computed, contradicting links coexist, and no link is
//! stamped or rewritten. `VerificationState::Disputed` stays for a person to
//! set by hand. Which link to keep is #172, and the rule is left open.
//!
//! So nothing here ranks by corroboration. #218 measured five duplicated pairs
//! out of 64 where the *false* link scores higher, worst case 0.22 against
//! 0.71, and a report sorted by the figure would put the wrong link first.

use clap::Args as ClapArgs;
use words_to_data::inspect::{self, ContradictionGroup};

use crate::load::{self, with_dataset};

/// How many groups the human output prints per category before it stops.
///
/// `--json` carries them all. A three-release-point corpus gives tens of
/// duplication groups, and a reader who wants every one is piping the output.
const SHOWN: usize = 20;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::contradictions(&d)),
        "Error reading the dataset's links",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    println!("Links read:   {}", report.totals.links_read);
    println!("Duplication:  {} subject(s)", report.totals.duplication);
    println!("Disagreement: {} subject(s)", report.totals.disagreement);

    print_category(
        "Duplication — one subject, one object, links in more than one window",
        &report.duplication,
    );
    print_category(
        "Disagreement — one subject, and links that name different objects",
        &report.disagreement,
    );

    if report.totals.duplication + report.totals.disagreement > 0 {
        // Said once, at the end, because it is the reading a reviewer is most
        // likely to take from a list like this and it is the wrong one.
        println!("\nNothing above says which link is right. That rule is #172.");
    }
}

/// Print one category, named and headed, whether or not it holds anything.
///
/// An empty category still prints its heading. "Duplication: none" and silence
/// read the same way on a terminal, and only the first says the command looked.
fn print_category(heading: &str, groups: &[ContradictionGroup]) {
    println!("\n{heading}:");
    if groups.is_empty() {
        println!("  none");
        return;
    }
    for group in groups.iter().take(SHOWN) {
        println!("  {} [{}]", group.subject, group.kind);
        for link in &group.links {
            let window = link
                .window
                .as_ref()
                .map(|window| window.to_string())
                .unwrap_or_else(|| "no window".to_string());
            let method = link.method.as_deref().unwrap_or("no method recorded");
            print!("    {window}  {} [{}]", link.source, method);
            match link.corroboration {
                Some(figure) => println!("  corroboration {figure:.2}"),
                None => println!(),
            }
            println!("      -> {}", link.object);
            // The change, where the object carries one. An amendment's reference
            // does not say what the link records, and two links naming one
            // amendment read as one row repeated without this.
            if let Some(change) = &link.change {
                println!("         {change}");
            }
        }
    }
    if groups.len() > SHOWN {
        println!(
            "  … {} more subject(s); pass --json for all of them",
            groups.len() - SHOWN
        );
    }
}
