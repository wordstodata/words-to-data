//! `words_to_data expressions` — list every expression with its size.

use clap::Args as ClapArgs;
use words_to_data::dataset::WorkId;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// List only this work, e.g. `uscode/title_9`
    #[arg(long)]
    pub work: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let work = args.work.as_deref().map(WorkId::new);
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let expressions = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::expressions(&d, work.as_ref())),
        "Error reading expressions",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&expressions).unwrap());
        return;
    }

    // The id carries work and date together, so one column says both, and it is
    // the form that goes straight back in on `--from` / `--to`.
    let width = expressions
        .iter()
        .map(|e| e.id.len())
        .max()
        .unwrap_or(0)
        .max("EXPRESSION".len());

    println!("{:<width$}  {:>10}  LABEL", "EXPRESSION", "ELEMENTS");
    for e in &expressions {
        println!(
            "{:<width$}  {:>10}  {}",
            e.id,
            e.element_count,
            e.label.as_deref().unwrap_or("")
        );
    }
    println!("{} expression(s)", expressions.len());
}
