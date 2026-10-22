//! `words_to_data release-points` — list the release points the mirror offers
//! and the cache holds.
//!
//! `build-dataset` and `add-release-points` take release-point dates, and this
//! is where a reader finds them. The listing reads the same source those
//! commands read, made the same way, so a date it says the mirror offers is a
//! date a build fetches. It only reads: nothing is downloaded and nothing is
//! written to the cache.

use clap::Args as ClapArgs;
use serde::Serialize;

use crate::release_points::{DEFAULT_MIRROR_INDEX, Listed, ReleaseSource};

#[derive(ClapArgs)]
pub struct Args {
    /// Mirror manifest URL
    #[arg(long, default_value = DEFAULT_MIRROR_INDEX)]
    pub mirror_index: String,

    /// Cache directory that holds downloaded release points
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    ///
    /// The listing then holds the cache alone, and does not say what the
    /// mirror offers.
    #[arg(long)]
    pub offline: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

/// The listing as `--json` prints it.
#[derive(Serialize)]
struct Listing<'a> {
    /// The cache directory read.
    cache_dir: String,
    /// The mirror manifest read, and `None` when the run is offline.
    mirror_index: Option<&'a str>,
    release_points: Vec<Listed>,
}

pub fn run(args: Args) {
    let source = crate::fail::or_exit(
        ReleaseSource::new(&args.mirror_index, args.cache_dir.as_deref(), args.offline),
        "Error fetching mirror manifest",
    );
    let listed = source.list();

    if args.json {
        let listing = Listing {
            cache_dir: source.cache_dir().display().to_string(),
            mirror_index: (!args.offline).then_some(args.mirror_index.as_str()),
            release_points: listed,
        };
        println!("{}", serde_json::to_string_pretty(&listing).unwrap());
        return;
    }

    let cache = source.cache_dir().display();
    if args.offline {
        println!("Release points in the cache at {cache}");
        println!("This run is offline: it lists the cache, not the mirror.");
    } else {
        println!(
            "Release points in the mirror ({}) and the cache at {cache}",
            args.mirror_index
        );
    }
    println!();
    for point in &listed {
        println!("{}  {}", point.date, where_it_is(point));
    }
    println!("{} release point(s)", listed.len());
}

/// Where one release point can be read from, in words.
fn where_it_is(point: &Listed) -> &'static str {
    match (point.in_mirror, point.in_cache) {
        (Some(true), true) => "mirror and cache",
        (Some(true), false) => "mirror only",
        (Some(false), _) => "cache only (the mirror no longer offers it)",
        (None, _) => "cache only",
    }
}
