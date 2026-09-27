//! `words_to_data amendment-addresses` — the Code address each amending
//! instruction of a public law acts on, or why it could not be read (#248).
//!
//! The address comes from the publisher's markup, which the dataset holds as
//! the bill's own document: the section the amending line names, the
//! designations its citation gives, and the containers the scope phrases open.
//! See [`words_to_data::uslm::amendment_address`].
//!
//! **It reads the dataset and nothing else.** No XML, no model call and no
//! diff, and nothing is stored: an address is derived from the bill the dataset
//! already holds (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! **Every instruction is printed.** One the resolver could not address comes
//! with its reason, so a gap is never silent.

use clap::Args as ClapArgs;
use words_to_data::uslm::amendment_address::{AmendmentAddress, addresses_in};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Which bill to read, as the dataset names it, such as `119-hr-1`
    #[arg(long)]
    pub bill: String,

    /// Show only this amendment, by its id or the start of it
    #[arg(long)]
    pub amendment: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let document = crate::fail::or_exit(
        with_dataset!(ds, d => d.bill_document(&args.bill)),
        "Error reading the bill",
    );
    let Some(document) = document else {
        eprintln!(
            "The dataset holds no document for bill {}. A dataset built before \
             the bill was stored as a document (#196) has none to read.",
            args.bill
        );
        std::process::exit(1);
    };

    let mut addresses = addresses_in(&args.bill, &document.root);
    if let Some(wanted) = &args.amendment {
        addresses.retain(|address| address.amendment_id.starts_with(wanted.as_str()));
        if addresses.is_empty() {
            eprintln!("Bill {} states no amendment {wanted}", args.bill);
            std::process::exit(1);
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&addresses).unwrap());
        return;
    }

    let addressed = addresses
        .iter()
        .filter(|address| address.section.is_some())
        .count();
    println!(
        "{} amendment(s): {addressed} addressed, {} not",
        addresses.len(),
        addresses.len() - addressed
    );
    for address in &addresses {
        print_address(address);
    }
}

/// One amendment: its id, then where it acts or why that is not known, then
/// where in the bill to read its words.
fn print_address(address: &AmendmentAddress) {
    let id = &address.amendment_id[..address.amendment_id.len().min(12)];
    match (&address.section, &address.unresolved) {
        (Some(section), _) => {
            let below: String = address
                .container
                .iter()
                .map(|step| format!("({})", step.number))
                .collect();
            println!("  {id}  {section}{below}");
        }
        (None, Some(reason)) => println!("  {id}  not addressed — {reason}"),
        (None, None) => println!("  {id}  not addressed"),
    }
    println!("    {}", address.path);
}
