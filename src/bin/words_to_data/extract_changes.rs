//! `words_to_data extract-changes` — extract word-level changes from every bill
//! amendment via an LLM and write them into the dataset.
//!
//! This is the nondeterministic, LLM-bound half of the pipeline. It runs over
//! all bills in one pass; scoring against a US Code diff happens separately and
//! deterministically in `score-amendments`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Args as ClapArgs;
use serde::{Deserialize, Serialize};
use words_to_data::dataset::{Dataset, Format};
use words_to_data::legislature::BillDiff;

use crate::llm::{ChatOptions, LlmClient};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset (compact JSON) containing the bills to extract changes from
    pub dataset: String,

    /// Base URL of an OpenAI-compatible chat-completions server
    #[arg(long, default_value = "http://localhost:8080")]
    pub base_url: String,

    /// Model name to request (llama.cpp ignores this; DeepSeek etc. require it)
    #[arg(long, default_value = "")]
    pub model: String,

    /// Number of concurrent LLM requests
    #[arg(long, default_value_t = 1)]
    pub threads: usize,

    /// Re-query the LLM for every amendment, ignoring any cached extractions
    #[arg(long)]
    pub no_cache: bool,

    /// Where to write the enriched dataset (defaults to overwriting the input)
    #[arg(long)]
    pub output: Option<String>,
}

/// One amendment's cached extraction.
///
/// Untagged so a cache written before replies were recorded still loads: it is
/// a cache, not a record, and forcing a re-extraction of everything to add a
/// field nobody had yet would be a poor trade.
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Cached {
    /// The current form: what was parsed, and what the model actually said.
    WithReply {
        changes: Vec<BillDiff>,
        reply: String,
        prompt_hash: String,
    },
    /// Written before replies were recorded. Usable, but carries no evidence.
    ChangesOnly(Vec<BillDiff>),
}

impl Cached {
    fn changes(&self) -> &[BillDiff] {
        match self {
            Self::WithReply { changes, .. } => changes,
            Self::ChangesOnly(changes) => changes,
        }
    }
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
    crate::load::refuse_sqlite(&args.dataset, "extract-changes");
    let mut dataset = crate::fail::or_exit(
        Dataset::load(&args.dataset, Format::Compact),
        "Error loading dataset",
    );

    // Amendments that already carry changes are done — don't re-extract or
    // re-apply them (applying twice would duplicate changes in the dataset).
    let mut done: HashSet<String> = HashSet::new();
    let mut amendments: Vec<Task> = Vec::new();
    for bill_id in dataset.list_bill_ids().expect("Error listing bills") {
        let bill = dataset
            .get_bill(&bill_id)
            .expect("Error reading bill")
            .expect("bill id from list_bill_ids should exist");
        for amendment in bill.amendments.values() {
            if amendment.changes.is_empty() {
                amendments.push(Task {
                    amendment_id: amendment.id.clone(),
                    amending_text: amendment.amending_text.clone(),
                });
            } else {
                done.insert(amendment.id.clone());
            }
        }
    }

    // The cache lets a fresh dataset reuse extractions from a previous run;
    // `--no-cache` forces every not-yet-done amendment to be re-queried.
    let cache_path = sibling(&args.dataset, "changes_cache.json");
    let mut cache = load_cache(&cache_path);
    let reuse = !args.no_cache;
    let todo: Vec<Task> = amendments
        .into_iter()
        .filter(|t| !(reuse && cache.contains_key(&t.amendment_id)))
        .collect();

    println!(
        "{} amendments already extracted; {} cached; {} to extract",
        done.len(),
        cache.len(),
        todo.len(),
    );

    if !todo.is_empty() {
        let llm = LlmClient::new(args.base_url.clone(), args.model.clone(), None);
        // The cache is updated and flushed to disk after every successful call so
        // an interrupted run can be resumed without losing completed extractions.
        let shared = Mutex::new(cache);
        extract_all(&llm, &todo, args.threads, &shared, &cache_path);
        cache = shared.into_inner().unwrap();
    }

    // Apply cached changes for every amendment not already populated.
    let model_name = if args.model.is_empty() {
        "local".to_string()
    } else {
        args.model.clone()
    };
    for (amendment_id, cached) in &cache {
        if done.contains(amendment_id) {
            continue;
        }
        for change in cached.changes() {
            dataset.add_changes_to_amendment(amendment_id, change);
        }

        // The amending text is a fact from the bill; these word-level changes
        // are a model's reading of it. Recording the evidence without the
        // verification state would read as corroboration for a machine guess.
        let evidence = match cached {
            Cached::WithReply {
                reply, prompt_hash, ..
            } => {
                let reply_id = crate::fail::or_exit(
                    dataset.add_reply(reply),
                    "Error recording the model reply",
                );
                Some(words_to_data::link::Evidence {
                    reasoning: None,
                    reply: Some(reply_id),
                    model: Some(model_name.clone()),
                    prompt_hash: Some(prompt_hash.clone()),
                })
            }
            Cached::ChangesOnly(_) => None,
        };
        dataset.set_amendment_provenance(
            amendment_id,
            words_to_data::link::Provenance {
                source: format!("model:{model_name}"),
                method: Some("extract-changes".to_string()),
                verification: words_to_data::link::VerificationState::MachineSuggested,
                evidence,
                raw_score: None,
                timestamp: Some(time::OffsetDateTime::now_utc()),
                corroboration: None,
            },
        );
    }

    let output = args.output.as_deref().unwrap_or(&args.dataset);
    dataset
        .save(output, Format::Compact)
        .expect("Error saving dataset");

    println!("Wrote {output}");
}

/// Run LLM extraction over every task, using `threads` OS worker threads.
///
/// Each successful extraction is inserted into `cache` and flushed to
/// `cache_path` immediately (under the lock), so an interrupted run leaves a
/// complete, resumable cache on disk. The dataset itself is untouched here.
fn extract_all(
    llm: &LlmClient,
    tasks: &[Task],
    threads: usize,
    cache: &Mutex<HashMap<String, Cached>>,
    cache_path: &Path,
) {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let worker_count = threads.max(1);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(task) = tasks.get(i) else { break };

                    match extract_changes(llm, &task.amending_text) {
                        Ok(cached) => {
                            // Insert and persist while holding the lock so the on-disk
                            // cache is always consistent with in-memory state.
                            let mut guard = cache.lock().unwrap();
                            guard.insert(task.amendment_id.clone(), cached);
                            write_cache(cache_path, &guard);
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
}

/// Query the LLM for one amendment and parse its `<response>` payload.
///
/// The reply is carried out rather than dropped. Nothing of the model's words
/// used to survive this command at all — the parsed result went to the cache
/// and the raw text only ever appeared in an error message (#58).
fn extract_changes(llm: &LlmClient, amending_text: &str) -> Result<Cached, String> {
    let user_prompt = format!(
        "Extract the word-level changes from this amendment text:\n\n<amendment>\n{amending_text}\n</amendment>"
    );
    let opts = ChatOptions {
        temperature: 1.0,
        max_tokens: Some(64_000),
    };
    let reply = llm.chat(EXTRACT_SYSTEM_PROMPT, &user_prompt, &opts)?;
    // Include the full raw model output on any parse failure so it can be inspected.
    let changes =
        parse_changes(&reply).map_err(|e| format!("{e}\n--- raw model output ---\n{reply}"))?;
    Ok(Cached::WithReply {
        changes,
        prompt_hash: prompt_hash(EXTRACT_SYSTEM_PROMPT, &user_prompt),
        reply,
    })
}

/// A hash of the exact prompt that was sent. The prompt itself is not stored:
/// it is built from the amending text, which the dataset already holds.
fn prompt_hash(system: &str, user: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(system.as_bytes());
    hasher.update([0u8]);
    hasher.update(user.as_bytes());
    hex::encode(hasher.finalize())
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
fn load_cache(path: &Path) -> HashMap<String, Cached> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).expect("Error parsing cache file"),
        Err(_) => HashMap::new(),
    }
}

/// Persist the extraction cache so a later run can resume without re-querying.
///
/// Writes to a temp file then renames, so an interrupt mid-write can never leave
/// a half-written (corrupt, unresumable) cache on disk.
fn write_cache(path: &Path, cache: &HashMap<String, Cached>) {
    let json = serde_json::to_string_pretty(cache).expect("Error serializing cache");
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).expect("Error writing cache");
    fs::rename(&tmp, path).expect("Error replacing cache");
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
