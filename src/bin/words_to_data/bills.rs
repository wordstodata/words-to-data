//! `words_to_data bills` — list every bill a dataset holds.
//!
//! `show-bill` takes an id, and until this existed nothing would tell you what
//! ids there were: `info` reports a count, and annotations carry ids but a
//! dataset has none until a matching step has run (#83).

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};
use crate::ui;

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
    let bills = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::bills(&d)),
        "Error reading bills",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&bills).unwrap());
        return;
    }

    // The id is the column that goes straight back in to `show-bill`, and so
    // is the public law.
    let laws: Vec<String> = bills
        .iter()
        .map(|b| public_law_cell(b.public_law.as_deref()))
        .collect();
    let width = bills
        .iter()
        .map(|b| b.bill_id.len())
        .max()
        .unwrap_or(0)
        .max("BILL".len());
    let law_width = laws
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(0)
        .max("PUBLIC LAW".len());

    println!(
        "{:<width$}  {:<law_width$}  {:>10}  {:>6}",
        ui::heading("BILL"),
        ui::heading("PUBLIC LAW"),
        ui::heading("AMENDMENTS"),
        ui::heading("LINKED")
    );
    for (b, law) in bills.iter().zip(&laws) {
        println!(
            "{:<width$}  {:<law_width$}  {:>10}  {:>6}",
            ui::figure(&b.bill_id),
            law,
            b.amendment_count,
            b.amendments_linked
        );
    }
    println!("{}", ui::quiet(format!("{} bill(s)", bills.len())));
    println!(
        "{}",
        ui::quiet("LINKED: the amendments that at least one standing amended_by link names.")
    );
}

/// The public law column: `Pub. L. 119-21`, or why there is none.
pub fn public_law_cell(law: Option<&str>) -> String {
    law.map_or_else(
        || "not enacted / not held".to_string(),
        |law| format!("Pub. L. {law}"),
    )
}
