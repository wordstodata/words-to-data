//! `words_to_data residue` — every amendment of a public law that no
//! `legislature.amended_by` link names, with the stage and the reason it
//! stopped (#251).
//!
//! See [`words_to_data::legislature::residue`] for what counts as unlinked and
//! where the reasons come from.
//!
//! **It stores nothing.** The list is derived each time, so an amendment leaves
//! it the moment a link names it, from the batch or through `link-amendment`.

use clap::Args as ClapArgs;
use serde::Serialize;
use words_to_data::legislature::residue::{Unlinked, unlinked_amendments};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// One bill, as the dataset names it, such as `119-hr-1`
    ///
    /// Every public law the dataset holds, when this is left out.
    #[arg(long)]
    pub bill: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

/// What `--json` prints.
#[derive(Serialize)]
struct Listing {
    rows: Vec<Unlinked>,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let rows = crate::fail::or_exit(
        with_dataset!(ds, d => unlinked_amendments(&d, args.bill.as_deref())),
        "Error reading the dataset's amendments",
    );

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&Listing { rows }).unwrap()
        );
    }
}
