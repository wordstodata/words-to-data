//! `words_to_data add-classifications` — store the OLRC's classification of
//! each public law a dataset holds, as `olrc.classified_from` links (#247).
//!
//! The Office of the Law Revision Counsel publishes, for each session of
//! Congress, which Code section each section of a public law was classified to.
//! This command reads the table for every public law the dataset holds, and
//! writes one `Asserted` link per section of the law a row names. What the
//! table says, and what this reader makes of it, is in `words_to_data::olrc`.
//!
//! **Every row is accounted for.** A row either states links, or it is printed
//! with the reason it states none. A title the dataset does not carry is out of
//! scope, never "not found".
//!
//! **Absence from a table is not absence of a change.** The OLRC lists only
//! what it classified to the Code, so a law with no rows is reported and not
//! read as a law that changed nothing.
//!
//! Tables are fetched live and cached. A page the cache holds is never fetched
//! again, and `--offline` reads the cache alone.

use std::collections::{BTreeMap, BTreeSet};

use clap::Args as ClapArgs;
use words_to_data::citation::resolve::SectionPaths;
use words_to_data::dataset::{Dataset, ExpressionId, Format, WorkId};
use words_to_data::olrc::{
    ClassificationRow, ClassificationTable, OlrcClient, SkipReason, classify, held_public_laws,
};
use words_to_data::storage::Storage;

/// The first Congress whose tables this command reads. Earlier tables are
/// published elsewhere, in another shape, and are out of scope for #247.
const FIRST_CONGRESS: u32 = 119;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite) holding public laws
    pub dataset: String,

    /// Cache directory for the OLRC's pages
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    #[arg(long)]
    pub offline: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which grows in
    /// place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "add-classifications",
        ))
    };

    let cache_dir = args.cache_dir.clone().map(std::path::PathBuf::from);
    let client = if args.offline {
        OlrcClient::offline(cache_dir.unwrap_or_else(|| {
            dirs::cache_dir()
                .expect("the user should have a cache directory")
                .join("words_to_data")
        }))
    } else {
        OlrcClient::new(cache_dir)
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            add_classifications(&mut dataset, &client);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            add_classifications(&mut dataset, &client);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }

    println!("OLRC pages fetched: {}", client.fetched());
}

/// The rows of one table page that name a law the dataset holds.
struct PageRows {
    url: String,
    rows: Vec<ClassificationRow>,
}

fn add_classifications<S: Storage>(dataset: &mut Dataset<S>, client: &OlrcClient) {
    let laws = crate::fail::or_exit(
        held_public_laws(dataset.storage()),
        "Error reading the dataset's works",
    );
    if laws.is_empty() {
        println!("This dataset holds no public law, so there is nothing to classify.");
        return;
    }

    let pages = read_tables(client, &laws);
    let paths = index_sections(dataset, &pages);
    let scope = crate::fail::or_exit(dataset.scope(), "Error reading the dataset's scope");

    let mut rows_per_law: BTreeMap<&str, usize> = BTreeMap::new();
    let mut stated = 0;
    let mut distinct = BTreeSet::new();
    let mut skipped = Vec::new();
    for page in &pages {
        for row in &page.rows {
            *rows_per_law.entry(row.public_law.as_str()).or_default() += 1;
        }
        let classified = classify(&page.rows, &scope, &paths, &format!("olrc:{}", page.url));
        for link in classified.links {
            stated += 1;
            distinct.insert(link.id());
            crate::fail::or_exit(dataset.add_link(link), "Error storing a classification");
        }
        skipped.extend(classified.skipped);
    }

    for law in &laws {
        match rows_per_law.get(law.as_str()) {
            Some(rows) => println!("{law}: {rows} row(s)"),
            None => println!(
                "{law}: no row(s). The OLRC lists only what it classified to the Code, \
                 so this is not a statement that the law changed nothing."
            ),
        }
    }

    let rows: usize = rows_per_law.values().sum();
    println!(
        "\n{} of {rows} row(s) stated {stated} link(s), which is {} distinct \
         olrc.classified_from link(s).",
        rows - skipped.len(),
        distinct.len()
    );
    println!("  A link is identified by what it says, so a second run stores none twice.");

    report_skipped(&skipped);
}

/// Read the table of every session that holds one of `laws`.
///
/// The 1st session first, and the 2nd only for a law the 1st does not list, so
/// no page is read that no law needs.
fn read_tables(client: &OlrcClient, laws: &[String]) -> Vec<PageRows> {
    let mut by_congress: BTreeMap<u32, BTreeSet<&str>> = BTreeMap::new();
    for law in laws {
        match law.split_once('-').and_then(|(c, _)| c.parse::<u32>().ok()) {
            Some(congress) if congress >= FIRST_CONGRESS => {
                by_congress.entry(congress).or_default().insert(law);
            }
            _ => println!(
                "{law}: not read. Tables before the {FIRST_CONGRESS}th Congress are \
                 published in another shape, and this command does not read them."
            ),
        }
    }

    let mut pages = Vec::new();
    for (congress, mut wanted) in by_congress {
        for session in [1, 2] {
            if wanted.is_empty() {
                break;
            }
            let page = crate::fail::or_exit(
                client.public_law_table(congress, session),
                "Error reading a classification table",
            );
            let table = crate::fail::or_exit(
                ClassificationTable::parse(&page.html),
                &format!("Error reading {}", page.url),
            );
            let rows: Vec<ClassificationRow> = table
                .rows
                .into_iter()
                .filter(|row| wanted.contains(row.public_law.as_str()))
                .collect();
            for row in &rows {
                wanted.remove(row.public_law.as_str());
            }
            pages.push(PageRows {
                url: page.url,
                rows,
            });
        }
    }
    pages
}

/// Where the sections of every title the rows name are, at every date the
/// dataset holds.
///
/// Every date and not only the latest: a section a law added is held only after
/// it, and a section a law repealed only before it. One expression at a time,
/// dropped once indexed, because a title of the Code is tens of thousands of
/// nodes.
fn index_sections<S: Storage>(dataset: &Dataset<S>, pages: &[PageRows]) -> SectionPaths {
    let titles: BTreeSet<&str> = pages
        .iter()
        .flat_map(|page| page.rows.iter().map(|row| row.title.as_str()))
        .collect();

    let mut paths = SectionPaths::new();
    for title in titles {
        let work = WorkId::new(format!("uscode/title_{title}"));
        let Ok(expressions) = dataset.expressions(&work) else {
            continue;
        };
        for held in expressions {
            let id = ExpressionId::new(work.clone(), held.id.at.clone());
            match dataset.get_expression(&id) {
                Ok(Some(expression)) => paths.add_work(&expression.root),
                _ => eprintln!("warning: {id} is listed and could not be read"),
            }
        }
    }
    paths
}

/// Print every row that stated no link, by reason.
fn report_skipped(skipped: &[words_to_data::olrc::Skipped]) {
    if skipped.is_empty() {
        println!("Every row stated a link.");
        return;
    }

    let mut not_held_by_title: BTreeMap<&str, usize> = BTreeMap::new();
    let mut missing_by_title: BTreeMap<&str, usize> = BTreeMap::new();
    let mut sections_not_held = Vec::new();
    for skip in skipped {
        match skip.reason {
            SkipReason::TitleNotHeld => {
                *not_held_by_title.entry(skip.row.title.as_str()).or_default() += 1
            }
            SkipReason::TitleMissing => {
                *missing_by_title.entry(skip.row.title.as_str()).or_default() += 1
            }
            SkipReason::SectionNotHeld => sections_not_held.push(&skip.row),
        }
    }

    println!("\n{} row(s) stated no link:", skipped.len());
    if !not_held_by_title.is_empty() {
        println!(
            "  {} name a title this dataset does not carry. Out of scope, not \"not found\":",
            not_held_by_title.values().sum::<usize>()
        );
        for (title, count) in &not_held_by_title {
            println!("    title {title}: {count}");
        }
    }
    if !missing_by_title.is_empty() {
        println!(
            "  {} name a title this dataset said it would carry and does not:",
            missing_by_title.values().sum::<usize>()
        );
        for (title, count) in &missing_by_title {
            println!("    title {title}: {count}");
        }
    }
    if !sections_not_held.is_empty() {
        println!(
            "  {} name a section the title does not hold at any date this dataset holds:",
            sections_not_held.len()
        );
        for row in sections_not_held {
            println!(
                "    {} U.S.C. {}  [{}]  Pub. L. {}, {}",
                row.title, row.section, row.description, row.public_law, row.law_sections
            );
        }
    }
}
