//! `words_to_data show-bill` — print a bill's amendments.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, OpenDataset, with_dataset};
use crate::ui;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Bill identifier (e.g. `119-hr-1`), or the public law it became
    /// (`Pub. L. 119-21`, `PL 119-21` or `119-21`)
    pub bill_id: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let bill_id = match inspect::public_law_named(&args.bill_id) {
        Some(law) => bill_of_law(&ds, &args.dataset, &law),
        None => args.bill_id.clone(),
    };
    crate::held::refuse_unless_bill_held(&ds, &args.dataset, &bill_id);
    let summary = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::show_bill(&d, &bill_id)),
        "Error reading bill",
    );

    let Some(summary) = summary else {
        eprintln!("Bill not found: {bill_id}");
        std::process::exit(1);
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        return;
    }

    println!(
        "{}       {}",
        ui::heading("Bill:"),
        ui::figure(&summary.bill_id)
    );
    println!(
        "{} {}",
        ui::heading("Public law:"),
        crate::bills::public_law_cell(summary.public_law.as_deref())
    );
    println!(
        "{} {}",
        ui::heading("Amendments:"),
        ui::figure(summary.amendment_count)
    );
    for a in &summary.amendments {
        let actions = a.action_types.join(", ");
        println!(
            "\n- {} {} {}",
            ui::figure(&a.id),
            ui::attention(format!("[{actions}]")),
            ui::quiet(format!("({} standing link(s))", a.link_count))
        );
        println!("    {}", truncate(&a.amending_text, 200));
    }
}

/// The bill that became public law `law`, or stop the run with exit 1 when the
/// dataset holds no document of that law.
fn bill_of_law(dataset: &OpenDataset, path: &str, law: &str) -> String {
    let bill = crate::fail::or_exit(
        with_dataset!(dataset, d => inspect::bill_of_public_law(d, law)),
        "Error reading the dataset's public laws",
    );
    bill.unwrap_or_else(|| {
        crate::fail::refuse(&format!(
            "This dataset holds no document of Pub. L. {law}, so no bill is known to have become it.\n\
             `words_to_data bills {path}` lists each bill it holds and its public law."
        ))
    })
}

/// Shorten `text` to `max` characters, adding an ellipsis when clipped.
fn truncate(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max).collect();
    format!("{clipped}…")
}
