//! `words_to_data score-amendments` — extract word-level changes from bill
//! amendments via an LLM, then score them against the US Code diff.
//!
//! Operates on an existing dataset (built by `build-dataset`) that already holds
//! the bills and the two US Code versions. The extracted changes are written
//! back into the dataset in place; the similarity scores are saved alongside it.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Args as ClapArgs;
use serde::Deserialize;
use words_to_data::dataset::{Dataset, Format};
use words_to_data::uslm::BillDiff;

use crate::llm::{ChatOptions, LlmClient};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset (compact JSON) containing the bills and both US Code versions
    pub dataset: String,

    /// Older US Code release-point date (YYYY-MM-DD)
    #[arg(long)]
    pub from_date: String,

    /// Newer US Code release-point date (YYYY-MM-DD)
    #[arg(long)]
    pub to_date: String,

    /// Base URL of an OpenAI-compatible chat-completions server
    #[arg(long, default_value = "http://localhost:8080")]
    pub base_url: String,

    /// Model name to request (llama.cpp ignores this; DeepSeek etc. require it)
    #[arg(long, default_value = "")]
    pub model: String,

    /// Number of concurrent LLM requests
    #[arg(long, default_value_t = 1)]
    pub threads: usize,

    /// Only keep similarity scores strictly above this cutoff
    #[arg(long, default_value_t = 0.4)]
    pub similarity_cutoff: f32,

    /// Re-query the LLM for every amendment, ignoring any cached extractions
    #[arg(long)]
    pub no_cache: bool,

    /// Where to write the enriched dataset (defaults to overwriting the input)
    #[arg(long)]
    pub output: Option<String>,
}

/// One amendment queued for LLM extraction.
struct Task {
    amendment_id: String,
    amending_text: String,
}

/// The `{"added": [...], "removed": [...]}` shape the model returns.
#[derive(Deserialize)]
struct RawDiff {
    #[serde(default)]
    added: Vec<String>,
    #[serde(default)]
    removed: Vec<String>,
}

pub fn run(args: Args) {
    let mut dataset = Dataset::load(&args.dataset, Format::Compact).expect("Error loading dataset");

    // Collect every amendment across all bills.
    let mut amendments: Vec<Task> = Vec::new();
    for bill_id in dataset.list_bill_ids().expect("Error listing bills") {
        let bill = dataset
            .get_bill(&bill_id)
            .expect("Error reading bill")
            .expect("bill id from list_bill_ids should exist");
        for amendment in bill.amendments.values() {
            amendments.push(Task {
                amendment_id: amendment.id.clone(),
                amending_text: amendment.amending_text.clone(),
            });
        }
    }

    // Always load prior extractions so a resumed run never loses work; `--no-cache`
    // only forces every amendment to be re-queried (the fresh result overwrites it).
    let cache_path = sibling(&args.dataset, "changes_cache.json");
    let mut cache = load_cache(&cache_path);
    let reuse = !args.no_cache;
    let todo: Vec<Task> = amendments
        .into_iter()
        .filter(|t| !(reuse && cache.contains_key(&t.amendment_id)))
        .collect();

    println!(
        "{} cached extractions loaded; {} amendments to extract",
        cache.len(),
        todo.len(),
    );

    if !todo.is_empty() {
        let llm = LlmClient::new(args.base_url, args.model, None);
        let extracted = extract_all(&llm, EXTRACT_SYSTEM_PROMPT, &todo, args.threads);
        cache.extend(extracted);
        write_cache(&cache_path, &cache);
    }

    // Apply extracted changes onto the amendments in the dataset.
    for (amendment_id, changes) in &cache {
        for change in changes {
            dataset.add_changes_to_amendment(amendment_id, change);
        }
    }

    // Score every amendment against the US Code diff.
    let diff = dataset
        .compute_diff(&args.from_date, &args.to_date)
        .expect("Error computing diff");

    let mut scores = Vec::new();
    for bill_id in dataset.list_bill_ids().expect("Error listing bills") {
        let bill = dataset.get_bill(&bill_id).unwrap().unwrap();
        scores.extend(diff.calculate_amendment_similarities(&bill).into_values());
    }
    scores.retain(|s| s.score > args.similarity_cutoff);
    scores.sort_by(|a, b| b.score.total_cmp(&a.score));

    // Persist scores next to the dataset, then write the enriched dataset itself.
    let scores_path = sibling(&args.dataset, "similarity_scores.json");
    fs::write(
        &scores_path,
        serde_json::to_string_pretty(&scores).expect("Error serializing scores"),
    )
    .expect("Error writing scores");

    let output = args.output.as_deref().unwrap_or(&args.dataset);
    dataset
        .save(output, Format::Compact)
        .expect("Error saving dataset");

    println!("Scored {} amendment matches above cutoff", scores.len());
    println!("Wrote {}", scores_path.display());
    println!("Wrote {output}");
}

/// Run LLM extraction over every task, using `threads` OS worker threads.
///
/// The dataset is untouched here — workers only read amendment text and return
/// `(amendment_id, changes)`, which the caller applies single-threaded.
fn extract_all(
    llm: &LlmClient,
    system_prompt: &str,
    tasks: &[Task],
    threads: usize,
) -> Vec<(String, Vec<BillDiff>)> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<(String, Vec<BillDiff>)>> = Mutex::new(Vec::new());
    let worker_count = threads.max(1);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(task) = tasks.get(i) else { break };

                    match extract_changes(llm, system_prompt, &task.amending_text) {
                        Ok(changes) => {
                            results
                                .lock()
                                .unwrap()
                                .push((task.amendment_id.clone(), changes));
                        }
                        Err(err) => {
                            eprintln!("ERROR extracting {}: {err}", task.amendment_id);
                        }
                    }

                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    println!("[{n}/{}] extracted", tasks.len());
                }
            });
        }
    });

    results.into_inner().unwrap()
}

/// Query the LLM for one amendment and parse its `<response>` payload.
fn extract_changes(
    llm: &LlmClient,
    system_prompt: &str,
    amending_text: &str,
) -> Result<Vec<BillDiff>, String> {
    let user_prompt = format!(
        "Extract the word-level changes from this amendment text:\n\n<amendment>\n{amending_text}\n</amendment>"
    );
    let opts = ChatOptions {
        temperature: 1.0,
        max_tokens: Some(64_000),
    };
    let raw = llm.chat(system_prompt, &user_prompt, &opts)?;
    // Include the full raw model output on any parse failure so it can be inspected.
    parse_changes(&raw).map_err(|e| format!("{e}\n--- raw model output ---\n{raw}"))
}

/// Pull the JSON array out of `<response>...</response>` and into `BillDiff`s.
fn parse_changes(raw: &str) -> Result<Vec<BillDiff>, String> {
    let start = raw
        .find("<response>")
        .ok_or("no <response> tag in model output")?
        + "<response>".len();
    let end = raw[start..]
        .find("</response>")
        .ok_or("no </response> tag in model output")?;
    let json_str = raw[start..start + end].trim();

    let raw_diffs: Vec<RawDiff> = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    Ok(raw_diffs
        .into_iter()
        .map(|d| BillDiff {
            added: d.added,
            removed: d.removed,
        })
        .collect())
}

/// Build a path to `filename` in the same directory as `dataset_path`.
fn sibling(dataset_path: &str, filename: &str) -> PathBuf {
    Path::new(dataset_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}

/// Load the extraction cache from disk, or start empty when it doesn't exist yet.
fn load_cache(path: &Path) -> HashMap<String, Vec<BillDiff>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).expect("Error parsing cache file"),
        Err(_) => HashMap::new(),
    }
}

/// Persist the extraction cache so a later run can resume without re-querying.
fn write_cache(path: &Path, cache: &HashMap<String, Vec<BillDiff>>) {
    fs::write(
        path,
        serde_json::to_string_pretty(cache).expect("Error serializing cache"),
    )
    .expect("Error writing cache");
}

/// System prompt instructing the model to extract word-level amendment changes.
const EXTRACT_SYSTEM_PROMPT: &str = r#"You are a precise legal text parser. When given amendment text from a bill, extract exact word-level changes described by each amendment instruction.

For each amendment instruction, compare the struck phrase and the inserted phrase word by word. Only report words that represent a net change — ignore words that appear in both the original and replacement phrases.

Return only valid JSON with no explanation, preamble, or markdown formatting.

OUTPUT FORMAT: and array of JSON objects, one for each identified amendment action. The output should be wrapped in XML <response> tags.

If there is a single amendment action:
<response>
[
{"removed": ["word1"], "added": ["word2"]}
]
</response>

If there are multiple amendment actions:
<response>
[
  {"removed": ["word1"], "added": ["word2"]},
  {"removed": [], "added": ["word3"]}
]
</response>

RULES:

1. STRIKING ONLY — "by striking X":
   {"removed": ["X"], "added": []}

2. INSERTING ONLY — "by inserting X" or "by adding X":
   {"removed": [], "added": ["X"]}

3. STRIKING AND INSERTING — "by striking X and inserting Y":
   Diff X and Y word by word. Only include words that changed.
   Example: striking "specified research" and inserting "foreign research"
   → {"removed": ["specified"], "added": ["foreign"]}
   (do NOT include "research" — it appears in both)

   Note: shared tokens must be excluded even when the struck phrase contains additional
   tokens before or after the shared ones. Always align and diff the full phrases, not
   just the differing region.

4. MULTIPLE ACTIONS in one amendment block:
   Return one object per action, in order, as an array.

5. CASING: Preserve the original casing of each word as it appears in the quoted text.

6. PUNCTUATION:
   - Tokenize by whitespace — each space-separated unit is one token
   - KEEP parentheses, hyphens, and brackets as part of whichever token they are attached to
     Example: "(as-stated" is one token, "81(e)(ii)))" is one token, "(something" is one token
   - OMIT tokens that are purely whitespace
   - OMIT standalone quotation marks that are not attached to a word
   - Do NOT treat a parenthesized clause as a single unit — each space-separated word inside is its own token, but opening/closing parens stay glued to their adjacent word

7. MULTI-WORD CHANGES: If multiple consecutive words change, include each as a separate string in the array.
   Example: striking "specified research expenses" and inserting "foreign experimental costs"
   → {"removed": ["specified", "expenses"], "added": ["foreign", "costs"]}
   (do NOT include "research" — it appears in both)

8. IDENTIFYING AMENDMENT ACTIONS:
   Amendment text is often structured with labeled clauses ((A), (B), (i), (ii), etc.).
   Each labeled clause typically contains one amendment action. Use these structural markers
   to identify action boundaries — do not merge actions from different clauses into one object,
   even if they appear in the same amendment block.

   Structural markers to treat as action boundaries:
   - Lettered clauses: (A), (B), (C), ...
   - Roman numeral clauses: (i), (ii), (iii), ...
   - Numbered clauses: (1), (2), (3), ...

   Each clause containing a "by striking", "by inserting", or "by adding" instruction
   produces exactly one object in the output array.

   Example: an amendment block with clauses (i) and (ii) each containing one instruction
   → output array has exactly 2 objects, in order

9. STOPWORD FILTERING:
   Omit common function words that carry no discriminating value for matching purposes.
   Filter out the following words (case-insensitive) from both "removed" and "added" arrays:

   a, an, the, and, or, of, in, to, by, at, on, with, is, are, was, were, be, been, being, it, its, as, all, each,
   into, up, out, do, does, did, have, has, had, they, them, their, there, this, these, those, that
"#;
