//! `words_to_data add-opinions` — put court opinions into a dataset, by API.
//!
//! Each opinion id names a case. The case becomes one expression of one work,
//! `judicial/case_<cluster>`, with each writing as a child, and every U.S. Code
//! citation in a writing's text becomes a `judicial.cites` link from that
//! writing (#53, `docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`).
//!
//! A case is read whole: the cluster, which carries the date the court filed
//! it, and every opinion record the cluster lists. So a case costs one request
//! for the cluster and one for each record, and an id of a case already read in
//! this run costs nothing. Every response is cached, so running this again
//! costs nothing.
//!
//! **This spends the maintainer's quota.** CourtListener allows 125 requests a
//! day, 50 an hour and 5 a minute, on a rolling window, so the client paces
//! itself and the run reports what it spent.

use std::collections::{BTreeMap, BTreeSet};

use clap::Args as ClapArgs;
use words_to_data::citation::resolve::{Resolution, SectionPaths, resolve};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::courtlistener::{
    ClusterRecord, CourtListenerClient, OpinionRecord, case_expression,
};
use words_to_data::dataset::{Dataset, ExpressionId, WorkId};
use words_to_data::judicial::{CaseFacts, OpinionFacts};
use words_to_data::storage::Storage;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to the dataset to add the opinions to (compact JSON or SQLite)
    pub dataset: String,

    /// CourtListener opinion ids, comma-separated. Each names its case, and the
    /// case is added whole: every writing the cluster lists
    #[arg(long, value_delimiter = ',', required = true)]
    pub opinions: Vec<u64>,

    /// Cache directory for CourtListener responses
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    ///
    /// A run that needs a record it has not got fails by name instead of
    /// spending a request. This is how the committed records are read.
    #[arg(long)]
    pub offline: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which grows in
    /// place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    // Where the result goes: `None` is SQLite, which grows in place. Compact
    // JSON must be told, and it is asked here rather than at the save, so a run
    // that has nowhere to put its result spends none of the maintainer's daily
    // 125 requests.
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "add-opinions",
        ))
    };

    let client = build_client(&args);

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            add_opinions(&mut dataset, &client, &args.opinions);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                crate::load::load_compact(&args.dataset),
                "Error loading dataset",
            );
            add_opinions(&mut dataset, &client, &args.opinions);
            crate::fail::or_exit(
                crate::load::save_compact(&dataset, output),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }

    println!("CourtListener requests spent: {}", client.requests_spent());
}

fn build_client(args: &Args) -> CourtListenerClient {
    let cache_dir = args.cache_dir.clone().map(std::path::PathBuf::from);

    if args.offline {
        let directory = cache_dir.unwrap_or_else(|| {
            dirs::cache_dir()
                .expect("the user should have a cache directory")
                .join("words_to_data")
        });
        return CourtListenerClient::offline(directory);
    }

    let token = std::env::var("COURTLISTENER_API_KEY").unwrap_or_else(|_| {
        eprintln!(
            "Fetching opinions needs COURTLISTENER_API_KEY. Get a token here: \
             https://www.courtlistener.com/profile/api-tokens/\n\
             To read only what is already cached, pass --offline."
        );
        std::process::exit(1);
    });
    CourtListenerClient::new(token, cache_dir)
}

/// Read one record through the client, or stop the run and say which.
fn opinion_record(client: &CourtListenerClient, id: u64) -> OpinionRecord {
    crate::fail::or_exit(
        client
            .opinion(id)
            .and_then(|json| OpinionRecord::from_json(&json)),
        &format!("Error reading opinion {id}"),
    )
}

/// Store the case of each opinion, then link every citation its writings carry.
fn add_opinions<S: Storage>(dataset: &mut Dataset<S>, client: &CourtListenerClient, ids: &[u64]) {
    // One case per cluster, however many of its writings were named. The ids
    // are read in the order given, so the cases are stored in that order.
    let mut records: BTreeMap<u64, OpinionRecord> = BTreeMap::new();
    let mut clusters: Vec<ClusterRecord> = Vec::new();
    for &id in ids {
        if records.contains_key(&id) {
            continue;
        }
        let opinion = opinion_record(client, id);
        let cluster_id = opinion.cluster_id.unwrap_or_else(|| {
            eprintln!("Opinion {id} names no cluster, so its case cannot be read");
            std::process::exit(1);
        });
        records.insert(id, opinion);
        if clusters.iter().any(|cluster| cluster.id == cluster_id) {
            continue;
        }
        let cluster = crate::fail::or_exit(
            client
                .cluster(cluster_id)
                .and_then(|json| ClusterRecord::from_json(&json)),
            &format!("Error reading cluster {cluster_id}"),
        );
        // The case is whole or it is not the case: a dissent stored without the
        // Court's opinion would answer for a case it is only part of.
        for sibling in cluster.opinion_ids() {
            records
                .entry(sibling)
                .or_insert_with(|| opinion_record(client, sibling));
        }
        clusters.push(cluster);
    }

    let mut stored: Vec<(Opinion, String)> = Vec::new();
    for cluster in &clusters {
        let opinions: Vec<OpinionRecord> = records
            .values()
            .filter(|record| record.cluster_id == Some(cluster.id))
            .cloned()
            .collect();
        let case = crate::fail::or_exit(
            case_expression(cluster, &opinions),
            &format!("Error building case {}", cluster.id),
        );
        for report in &case.matter_reports {
            report.print_to_stderr();
        }
        let expression = &case.expression;
        println!(
            "{} @ {}  {}",
            expression.id.work,
            expression.id.at,
            cluster.case_name.as_deref().unwrap_or("unnamed"),
        );
        for writing in &case.writings {
            // A character reference this build cannot decode changes the words
            // of the opinion, and one of those characters is the section sign
            // every U.S.C. citation needs. Never silent.
            writing.report.print_to_stderr();
            let node = expression.root.find(&writing.path);
            let text = node
                .and_then(|node| node.data.content.as_deref())
                .unwrap_or_default()
                .to_string();
            let facts = node.and_then(|node| OpinionFacts::of(&node.data));
            println!(
                "  {}  {}  [{} / {} / {:?}]  {} chars",
                writing.path,
                facts
                    .and_then(|facts| facts.opinion_type)
                    .as_deref()
                    .unwrap_or("writing type not recorded"),
                writing.source.field,
                writing.source.method,
                writing.source.verification,
                text.chars().count(),
            );
            stored.push((
                Opinion::held(
                    writing.path.clone(),
                    writing.opinion_id.to_string(),
                    cluster.display(),
                ),
                text,
            ));
        }
        let sources = &cluster_sources(&case.expression);
        if !sources.is_empty() {
            println!(
                "  combined record(s) {sources} hold the same words as the writings: \
                 kept as a source, not stored as text"
            );
        }
        crate::fail::or_exit(
            dataset.add_expression(case.expression),
            &format!("Error storing case {}", cluster.id),
        );
    }

    link_citations(dataset, &stored);
}

/// The combined records a case keeps only as sources, as a list to print.
fn cluster_sources(expression: &words_to_data::dataset::Expression) -> String {
    CaseFacts::of(&expression.root.data)
        .map(|facts| facts.combined_records)
        .unwrap_or_default()
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Read every U.S. Code citation out of each opinion and write the links.
fn link_citations<S: Storage>(dataset: &mut Dataset<S>, stored: &[(Opinion, String)]) {
    // What each opinion cites, read once. The text is the text that was stored,
    // so an offset in a link means the same thing as an offset in the file.
    let found: Vec<(&Opinion, Vec<usc::UscCitation>)> = stored
        .iter()
        .map(|(opinion, text)| {
            let (citations, report) = usc::find_with_report(text);
            report.print_to_stderr();
            (opinion, citations)
        })
        .collect();

    let titles: BTreeSet<WorkId> = found
        .iter()
        .flat_map(|(_, citations)| citations.iter().map(|citation| citation.work()))
        .collect();

    let paths = index_sections(dataset, &titles);
    let scope = crate::fail::or_exit(dataset.scope(), "Error reading the dataset's scope");

    let mut stated = 0;
    let mut distinct = BTreeSet::new();
    let mut out_of_scope = 0;
    let mut absent = 0;
    let mut on_a_stub = 0;
    for (opinion, citations) in &found {
        for citation in citations {
            let cited = resolve(citation, &scope, &paths);
            for section in &cited {
                match section.resolution {
                    Resolution::OutOfScope | Resolution::Gap => out_of_scope += 1,
                    Resolution::Absent => absent += 1,
                    Resolution::Provision { .. } => {}
                }
            }
            for link in cites_links(opinion, citation, &cited) {
                distinct.insert(link.id());
                stated += 1;
                if link
                    .object
                    .path()
                    .and_then(|path| paths.stub_at(path))
                    .is_some()
                {
                    on_a_stub += 1;
                }
                crate::fail::or_exit(dataset.add_link(link), "Error storing a citation link");
            }
        }
    }

    // Both figures, because they are different facts. A link is identified by
    // what it says, so one case citing one section five times states the link
    // five times and the dataset holds it once
    // (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    println!(
        "\n{stated} citation(s) resolved to a provision, which is {} distinct \
         judicial.cites link(s).",
        distinct.len()
    );
    // A stub is a real node, so its link looks as good as any other (#310).
    println!(
        "{on_a_stub} of them land on a stub: a section whose heading in the latest \
         printing held says Repealed, Renumbered, Transferred or Omitted. `path` \
         and `cases-citing` give the heading."
    );
    println!(
        "{out_of_scope} cited section(s) name a title this dataset does not carry, and \
         {absent} name a section the title does not hold. Neither is \"not found\": \
         the first is out of scope and the second is a statement about the law."
    );
}

/// Where the sections of every cited title sit, for the titles the dataset holds.
///
/// One title at a time, and the tree is dropped as soon as it is indexed: a title
/// of the Code is tens of thousands of nodes and there is no reason to hold two.
/// The latest printing held is the one indexed, because that is where a provision
/// is now.
fn index_sections<S: Storage>(dataset: &Dataset<S>, titles: &BTreeSet<WorkId>) -> SectionPaths {
    let mut paths = SectionPaths::new();

    for work in titles {
        let Ok(expressions) = dataset.expressions(work) else {
            continue;
        };
        let Some(latest) = expressions.last() else {
            continue;
        };
        let id = ExpressionId::new(work.clone(), latest.id.at.clone());
        match dataset.get_expression(&id) {
            Ok(Some(expression)) => paths.add_work(&expression.root),
            _ => eprintln!("warning: {id} is listed and could not be read"),
        }
    }

    paths
}
