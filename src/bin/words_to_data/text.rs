//! `words_to_data text` — a Code provision and its children at one date, as
//! the law reads (#385).
//!
//! `search "" --path <p> --at <date> --snippet 0 --all` was the only way to
//! read the words, and it printed one hit for each field, with the full path
//! on each and no numbers.

use std::io::Write;

use clap::Args as ClapArgs;
use words_to_data::document::NodeType;
use words_to_data::query::{Locator, PathMatch};
use words_to_data::uslm::path::covers_path;
use words_to_data::uslm::text::{DEFAULT_LINE_LIMIT, ProvisionText, provision_text};

use crate::load::{self, with_dataset};

/// The root of every Code path.
const CODE: &str = NodeType::USCODE;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Structural path of a Code provision, e.g. `uscode/title_9/chapter_1/section_2`
    pub path: String,

    /// The date of the expression to read, e.g. `2025-07-30`
    #[arg(long, value_name = "DATE", value_parser = crate::span::publication_date)]
    pub at: String,

    /// How many lines to print before stopping. The answer says how many nodes it did not show
    #[arg(long, value_name = "LINES", default_value_t = DEFAULT_LINE_LIMIT)]
    pub limit: usize,

    /// Print only this many levels below the path. The last level prints its number and heading only
    ///
    /// `--depth 1` on a chapter lists its sections and their headings
    #[arg(long, value_name = "LEVELS")]
    pub depth: Option<usize>,

    /// Emit JSON instead of human-readable text: the same nodes, as a tree
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let path = with_dataset!(&ds, d => crate::held::path_argument(d, &args.path));
    // A court writing and a bill have other text fields and other numbers.
    // They are out of scope for now (#385).
    if !covers_path(CODE, &path) {
        crate::fail::refuse(&format!(
            "text supports Code paths only (`{CODE}/...`), and {path} is not one.\n\
             `words_to_data search {} \"\" --path {path} --snippet 0` prints the words of any path.",
            args.dataset
        ));
    }
    let found = crate::fail::or_exit(
        with_dataset!(&ds, d => provision_text(d, &path, &args.at, args.depth, args.limit)),
        "Error reading the provision",
    );
    // The notice `search` gives for the same path and date, from the same
    // readers, so the two commands cannot come to say two things.
    let Some(text) = found else {
        let locator = Locator::new()
            .in_window(&args.at, &args.at)
            .at_path(&path, PathMatch::Exact);
        let notice = crate::search::not_held(&ds, &locator, &args.dataset, true)
            .unwrap_or_else(|| format!("No expression dated {} holds {path}.", args.at));
        crate::fail::refuse(&notice);
    };

    if let Err(error) = report(&text, args.json)
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        crate::fail::refuse(&format!("Error writing the answer: {error}"));
    }
}

/// Write the answer to stdout, and say what the limit left out.
fn report(text: &ProvisionText, json: bool) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    if json {
        writeln!(out, "{}", serde_json::to_string_pretty(text).unwrap())?;
        return out.flush();
    }
    writeln!(out, "{} at {}, in {}", text.path, text.at, text.expression)?;
    for line in text.node.lines() {
        writeln!(out, "{line}")?;
    }
    if text.truncated {
        writeln!(
            out,
            "… {} more node(s) not shown; give --depth or a deeper path",
            text.nodes_not_shown
        )?;
    }
    out.flush()
}
