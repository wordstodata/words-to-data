//! `words_to_data match-amendments` — match bill amendments to US Code changes.
//!
//! For each amendment we gather candidate diffs (via pre-computed similarity
//! scores + section-mention scans), ask an LLM which candidate(s) the amendment
//! actually caused, and record the answer as a `ChangeAnnotation` written back
//! into the dataset in place.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Args as ClapArgs;
use serde::{Deserialize, Serialize};
use words_to_data::annotation::{
    AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation,
};
use words_to_data::dataset::{Dataset, Format};
use words_to_data::document::TextContentField;
use words_to_data::legislature::AmendingAction;
use words_to_data::link::{Evidence, Link};
use words_to_data::matching::{
    AmendmentMatch, Candidate, DEFAULT_SIMILARITY_CUTOFF, build_matches,
};
use words_to_data::storage::{LegislatureReader, Storage};

use crate::span::Span;
use words_to_data::llm::{ChatOptions, LlmAnnotation, LlmClient};

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON) that already has amendment changes + expressions
    pub dataset: String,

    #[command(flatten)]
    pub span: Span,

    /// Base URL of an OpenAI-compatible chat-completions server
    #[arg(long, default_value = "http://localhost:8080")]
    pub base_url: String,

    /// Model name to request (llama.cpp ignores this; DeepSeek etc. require it)
    #[arg(long, default_value = "")]
    pub model: String,

    /// API key for a hosted endpoint (DeepSeek and the like)
    ///
    /// Prefer the `W2D_API_KEY` environment variable: a key passed as a flag
    /// lands in shell history and in `ps`. A local llama.cpp server needs none.
    #[arg(long)]
    pub api_key: Option<String>,

    /// Extra parameters for the endpoint, as `key=value` (repeatable)
    ///
    /// Sent verbatim in the request body, so a provider-specific switch this
    /// build has never heard of still gets through — for example
    /// `--llm-param reasoning_effort=low`. The value is read as JSON when it
    /// parses as JSON, and as a plain string otherwise.
    #[arg(long = "llm-param", value_name = "KEY=VALUE")]
    pub llm_params: Vec<String>,

    /// Sampling temperature
    #[arg(long)]
    pub temperature: Option<f32>,

    /// Cap on the tokens the model may generate
    ///
    /// A reasoning model with no ceiling can think for a very long time.
    #[arg(long)]
    pub max_tokens: Option<u64>,

    /// Number of concurrent LLM requests
    #[arg(long, default_value_t = 1)]
    pub threads: usize,

    /// Only offer the model candidates scoring strictly above this cutoff
    #[arg(long, default_value_t = DEFAULT_SIMILARITY_CUTOFF)]
    pub similarity_cutoff: f32,

    /// Re-query the LLM for every amendment, ignoring any cached replies
    #[arg(long)]
    pub no_cache: bool,

    /// Where to write the annotated dataset (defaults to overwriting the input)
    #[arg(long)]
    pub output: Option<String>,
}

/// One work's candidate view, tagged with the pair it was built from.
///
/// A corpus run covers many works, and the candidates carry no work of their
/// own, so a flat list would mix title 26's with title 51's.
#[derive(Serialize)]
struct CandidatesOfWork {
    work: String,
    from: String,
    to: String,
    amendments: Vec<serde_json::Value>,
}

/// Everything one run needs apart from the dataset it works on.
///
/// The dataset is the one thing that differs between the two backends, so it is
/// the one thing passed separately.
struct Matching<'a> {
    args: &'a Args,
    llm: &'a LlmClient,
    opts: &'a ChatOptions,
    model_name: &'a str,
    annotator: &'a str,
    cache: &'a Mutex<Cache>,
    cache_path: &'a Path,
}

pub fn run(args: Args) {
    // Where the result goes: `None` is a database, which is changed in place.
    // A W2D file is read into memory, so the result has to be written out
    // again.
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(args.output.as_deref().unwrap_or(&args.dataset))
    };

    let opts = crate::fail::or_exit(
        chat_options(
            &args.llm_params,
            args.temperature.unwrap_or(0.0),
            args.max_tokens,
        ),
        "Error reading --llm-param",
    );
    let llm = LlmClient::new(
        args.base_url.clone(),
        args.model.clone(),
        words_to_data::llm::api_key_from(args.api_key.as_deref()),
    );
    let model_name = if args.model.is_empty() {
        "local".to_string()
    } else {
        args.model.clone()
    };
    let annotator = format!("model:{model_name}");

    // A reply already bought for a question is reused rather than bought
    // again, as `extract-changes` has always done with its own cache.
    let cache_path = sibling(&args.dataset, "matches_cache.json");
    let cache = Mutex::new(load_cache(&cache_path));

    let matching = Matching {
        args: &args,
        llm: &llm,
        opts: &opts,
        model_name: &model_name,
        annotator: &annotator,
        cache: &cache,
        cache_path: &cache_path,
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            matching.apply(&mut dataset);
            println!("Wrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            matching.apply(&mut dataset);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("Wrote {output}");
        }
    }
}

impl Matching<'_> {
    /// Ask the model about every amendment in the span, and write what it
    /// answers into the dataset.
    fn apply<S: Storage + LegislatureReader>(&self, dataset: &mut Dataset<S>) {
        let args = self.args;
        let pairs = args.span.resolve(&*dataset);
        let mut candidates_by_work = Vec::new();
        let mut applied = 0;
        let mut annotated_paths = 0;
        let mut reused = 0;
        let mut queried = 0;
        // Amendments whose reply never parsed, gathered across every pair in the
        // span so one run reports one total.
        let mut failed: Vec<String> = Vec::new();

        for (from, to) in pairs {
            let diff =
                crate::fail::or_exit(dataset.compute_diff(&from, &to), "Error computing diff");

            let matches = build_matches(&*dataset, &diff, args.similarity_cutoff);
            println!("\n{from} -> {to}");
            print_stats(&matches);

            // Split the amendments into the ones a cached reply already answers
            // and the ones the model still has to be asked about. `--no-cache`
            // sends every one of them to the model.
            let mut answered: Vec<(usize, Classification)> = Vec::new();
            let mut todo: Vec<Task> = Vec::new();
            for (index, m) in matches.iter().enumerate() {
                let question = question(m);
                let cached = if args.no_cache {
                    None
                } else {
                    cached_classification(self.cache, &question)
                };
                match cached {
                    Some(classification) => answered.push((index, classification)),
                    None => todo.push(Task {
                        match_index: index,
                        amendment_id: m.amendment_id.clone(),
                        question,
                    }),
                }
            }
            println!("{} cached; {} to query", answered.len(), todo.len());
            reused += answered.len();
            queried += todo.len();

            // Ask the LLM which candidate(s) each remaining amendment matches.
            let (matched, lost) = classify_all(
                self.llm,
                &todo,
                args.threads,
                self.cache,
                self.cache_path,
                self.opts,
            );
            failed.extend(lost);
            answered.extend(matched);

            // Apply the LLM's annotations single-threaded.
            for (match_idx, classification) in answered {
                let m = &matches[match_idx];
                // One reply produced every annotation below, so it is recorded once
                // and referenced, not copied onto each.
                let reply_id = crate::fail::or_exit(
                    dataset.add_reply(&classification.reply),
                    "Error recording the model reply",
                );
                for ann in classification.annotations {
                    let Some(candidate) = usize::try_from(ann.candidate_index)
                        .ok()
                        .and_then(|i| m.candidates.get(i))
                    else {
                        continue;
                    };

                    let annotation = ChangeAnnotation {
                        // A model answers in the drafter's words, so the reading is
                        // `from_prose`: `strike` becomes the schema's `delete` and
                        // `strike and insert` its `substitute`. A word neither
                        // vocabulary holds still falls back to `Amend`, because an
                        // annotation must name an action, but it is said out loud
                        // first. `Amend` on its own would be a quiet lie about what
                        // the law did.
                        operation: ann
                            .operation
                            .as_deref()
                            .and_then(read_operation)
                            .unwrap_or(AmendingAction::Amend),
                        source_bill: BillReference {
                            bill_id: m.bill_id.clone(),
                            amendment_id: m.amendment_id.clone(),
                            causative_text: ann
                                .causative_text
                                .clone()
                                .unwrap_or_else(|| m.amending_text.clone()),
                        },
                        paths: vec![candidate.diff.root_path.clone()],
                        metadata: AnnotationMetadata {
                            status: AnnotationStatus::Pending,
                            confidence: ann.confidence,
                            annotator: self.annotator.to_string(),
                            timestamp: time::OffsetDateTime::now_utc(),
                            notes: None,
                            reasoning: ann.reasoning,
                        },
                    };
                    // Links are what is stored, so this writes links rather than
                    // handing an annotation to a convenience that fans out. One
                    // annotation is one link per path it names.
                    for mut link in Link::from_annotation(&annotation, &from, &to) {
                        link.provenance.evidence = Some(Evidence {
                            reasoning: annotation.metadata.reasoning.clone(),
                            reply: Some(reply_id.clone()),
                            model: Some(self.model_name.to_string()),
                            prompt_hash: Some(classification.prompt_hash.clone()),
                        });
                        crate::fail::or_exit(dataset.add_link(link), "Error adding link");
                    }
                    applied += 1;
                }
            }

            annotated_paths += dataset.annotated_paths(&from, &to).len();
            candidates_by_work.push(CandidatesOfWork {
                work: from.work.to_string(),
                from: from.to_string(),
                to: to.to_string(),
                amendments: candidates_view(&matches),
            });
        }

        // Persist the candidate view next to the dataset for inspection.
        let candidates_path = sibling(&args.dataset, "candidates.json");
        crate::fail::or_exit(
            fs::write(
                &candidates_path,
                serde_json::to_string_pretty(&candidates_by_work)
                    .expect("Error serializing candidates"),
            ),
            "Error writing candidates.json",
        );

        println!(
            "\nApplied {applied} annotation(s) across {} work(s)",
            candidates_by_work.len()
        );
        println!("{reused} replies reused; {queried} model calls made");
        println!("Annotated paths: {annotated_paths}");
        crate::report::failed_amendments(&failed);
        println!("Wrote {}", candidates_path.display());
    }
}

fn print_stats(matches: &[AmendmentMatch]) {
    let total = matches.len();
    if total == 0 {
        println!("No amendments with candidates found");
        return;
    }
    let counts: Vec<usize> = matches.iter().map(|m| m.candidates.len()).collect();
    let total_candidates: usize = counts.iter().sum();
    let one = counts.iter().filter(|&&c| c == 1).count();
    let many = counts.iter().filter(|&&c| c > 1).count();
    println!("Total amendments with candidates: {total}");
    println!("Total candidates: {total_candidates}");
    println!(
        "Avg candidates/amendment: {:.2}",
        total_candidates as f64 / total as f64
    );
    println!("Amendments with 1 candidate: {one}");
    println!("Amendments with 2+ candidates: {many}");
}

/// Run LLM classification over every task using `threads` OS worker threads.
///
/// Workers only read their task and return `(match_index, classification)`; the
/// caller applies the annotations to the dataset single-threaded.
/// Each successful reply is inserted into `cache` and flushed to `cache_path`
/// immediately (under the lock), so a run interrupted part-way leaves a cache
/// the next run can resume from.
/// Returns the classifications that succeeded, and the ids of the amendments
/// that failed so the caller can report the loss. A reply that does not parse
/// is not kept: it backs no statement, so it is not evidence
/// (`docs/adr/0005`). Running the command again is the retry.
fn classify_all(
    llm: &LlmClient,
    tasks: &[Task],
    threads: usize,
    cache: &Mutex<Cache>,
    cache_path: &Path,
    opts: &ChatOptions,
) -> (Vec<(usize, Classification)>, Vec<String>) {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Classification)>> = Mutex::new(Vec::new());
    let failed: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let worker_count = threads.max(1);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(task) = tasks.get(i) else { break };

                    let outcome = match classify(llm, &task.question, opts) {
                        Ok(classification) => {
                            // Insert and persist while holding the lock so the
                            // on-disk cache is always consistent with memory.
                            let mut guard = cache.lock().unwrap();
                            guard.insert(
                                task.question.candidate_hash.clone(),
                                Cached {
                                    reply: classification.reply.clone(),
                                    prompt_hash: classification.prompt_hash.clone(),
                                },
                            );
                            write_cache(cache_path, &guard);
                            drop(guard);
                            results
                                .lock()
                                .unwrap()
                                .push((task.match_index, classification));
                            "matched"
                        }
                        Err(err) => {
                            // One call, so the raw reply inside `err` stays in one
                            // piece even when several workers fail at once.
                            eprintln!("ERROR matching {}: {err}", task.amendment_id);
                            failed.lock().unwrap().push(task.amendment_id.clone());
                            "failed"
                        }
                    };

                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    println!("[{n}/{}] {outcome}", tasks.len());
                }
            });
        }
    });

    // Workers race, so sort for a report that reads the same on every run.
    let mut failed = failed.into_inner().unwrap();
    failed.sort();
    (results.into_inner().unwrap(), failed)
}

/// One amendment's answer: what the model said, and what it was parsed into.
///
/// The reply is carried out of here rather than dropped. It is what lets a
/// receiving party check that the parse was faithful, and it is the only thing
/// that can prove the parser survives what a model really emits (#58).
struct Classification {
    annotations: Vec<LlmAnnotation>,
    reply: String,
    prompt_hash: String,
}

/// Query the LLM for one amendment and parse its annotation list.
fn classify(
    llm: &LlmClient,
    question: &Question,
    opts: &ChatOptions,
) -> Result<Classification, String> {
    let reply = llm.chat(SYSTEM_PROMPT, &question.user_prompt, opts)?;
    let annotations = words_to_data::llm::parse_annotations(&reply)
        .map_err(|e| format!("{e}\n--- raw model output ---\n{reply}"))?;
    Ok(Classification {
        annotations,
        prompt_hash: question.prompt_hash.clone(),
        reply,
    })
}

/// One amendment queued for a model call.
struct Task {
    /// Where the answer belongs in the run's match list.
    match_index: usize,
    amendment_id: String,
    question: Question,
}

/// One amendment's question: the prompt to send, and the two hashes a reply to
/// it is filed under.
struct Question {
    user_prompt: String,
    /// What the model is asked about.
    candidate_hash: String,
    /// The words it is asked in.
    prompt_hash: String,
}

/// Build the question for one amendment.
fn question(m: &AmendmentMatch) -> Question {
    let user_prompt = build_user_prompt(m);
    Question {
        candidate_hash: candidate_hash(m),
        prompt_hash: prompt_hash(SYSTEM_PROMPT, &user_prompt),
        user_prompt,
    }
}

/// A hash of what the model is asked about: the amendment, and every candidate
/// offered for it, in the order it reads them.
fn candidate_hash(m: &AmendmentMatch) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(m.amendment_id.as_bytes());
    for candidate in &m.candidates {
        hasher.update([0u8]);
        hasher.update(serde_json::to_vec(&candidate.diff).expect("a diff should serialize"));
    }
    hex::encode(hasher.finalize())
}

/// One amendment's cached answer: what the model said, and what it answered.
///
/// Only the reply is kept, not the annotations read out of it. The reply is
/// what the model sent, and the annotations are a reading of it, so a later run
/// reads it again rather than trust a summary of it.
#[derive(Clone, Serialize, Deserialize)]
struct Cached {
    reply: String,
    prompt_hash: String,
}

/// Replies bought by earlier runs, by a hash of the candidates each answered
/// for. `changes_cache.json` is the same file for `extract-changes`.
type Cache = HashMap<String, Cached>;

/// The reply an earlier run bought for this question, if there is one.
///
/// The key is the pair, not the candidates alone: a reply is only reused when
/// the prompt recorded beside it is the prompt this build now sends. A
/// candidate hash on its own would hand back an answer to a question nobody
/// asked, and nothing in the dataset would show it (#123).
///
/// A cached reply that no longer parses is dropped rather than kept, so the
/// amendment is queried again. A reply that parses into no statement is not
/// evidence (`docs/adr/0005`), and that holds when it comes off the disk too.
fn cached_classification(cache: &Mutex<Cache>, question: &Question) -> Option<Classification> {
    let cached = cache
        .lock()
        .unwrap()
        .get(&question.candidate_hash)
        .cloned()?;
    if cached.prompt_hash != question.prompt_hash {
        return None;
    }
    let annotations = words_to_data::llm::parse_annotations(&cached.reply).ok()?;
    Some(Classification {
        annotations,
        prompt_hash: cached.prompt_hash,
        reply: cached.reply,
    })
}

/// Load the reply cache from disk, or start empty when it does not exist yet.
fn load_cache(path: &Path) -> Cache {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).expect("Error parsing cache file"),
        Err(_) => Cache::new(),
    }
}

/// Persist the reply cache so a later run can resume without re-querying.
///
/// Writes to a temp file then renames, so an interrupt mid-write can never
/// leave a half-written (corrupt, unresumable) cache on disk.
fn write_cache(path: &Path, cache: &Cache) {
    let json = serde_json::to_string_pretty(cache).expect("Error serializing cache");
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).expect("Error writing cache");
    fs::rename(&tmp, path).expect("Error replacing cache");
}

/// A hash of the exact prompt that was sent.
///
/// The prompt itself is not stored: it is built from material the dataset
/// already holds, so keeping it would duplicate the file's own contents. The
/// hash still answers the question that matters — whether the prompt behind
/// this reply is the one this build produces now.
fn prompt_hash(system: &str, user: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(system.as_bytes());
    hasher.update([0u8]);
    hasher.update(user.as_bytes());
    hex::encode(hasher.finalize())
}

/// Build the user prompt: the amendment plus its formatted candidates.
fn build_user_prompt(m: &AmendmentMatch) -> String {
    let action_types = if m.action_types.is_empty() {
        "Unknown".to_string()
    } else {
        m.action_types
            .iter()
            .map(action_str)
            .collect::<Vec<_>>()
            .join(", ")
    };

    let candidates_text = m
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| format_candidate(i, c))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "## Amendment\n\n**Action Types**: {action_types}\n\n**Amending Text**:\n{}\n\n---\n\n## Candidates ({} total)\n\n{candidates_text}\n\n---\n\nAnalyze the amendment and identify which candidate(s) match. Return JSON.",
        m.amending_text,
        m.candidates.len(),
    )
}

/// Format a single candidate for inclusion in the prompt.
fn format_candidate(index: usize, candidate: &Candidate) -> String {
    let diff = &candidate.diff;
    let mut lines = vec![
        format!("### Candidate {index}"),
        format!("**Location**: {}", diff.root_path),
    ];

    match &candidate.similarity {
        Some(s) => lines.push(format!(
            "**Similarity**: score={:.2}, precision={:.2}, recall={:.2}",
            s.score, s.precision, s.recall
        )),
        None => lines.push("**Similarity**: (not computed)".to_string()),
    }

    if !candidate.mentions.is_empty() {
        let texts: Vec<&str> = candidate
            .mentions
            .iter()
            .map(|m| m.matched_text.as_str())
            .collect();
        lines.push(format!("**Mentions**: {texts:?}"));
    }

    if !diff.changes.is_empty() {
        lines.push(String::new());
        lines.push("**Changes**:".to_string());
        for change in &diff.changes {
            let old_value = trimmed_or_empty(&change.old_value);
            let new_value = trimmed_or_empty(&change.new_value);
            lines.push(format!("- {}:", field_name(&change.field_name)));
            lines.push(format!("   * old: {old_value}"));
            lines.push(format!("   * new: {new_value}"));
        }
    }

    push_elements(&mut lines, "Added Elements", &diff.added);
    push_elements(&mut lines, "Removed Elements", &diff.removed);

    lines.join("\n")
}

/// Append an "Added/Removed Elements" section listing each element's text fields.
fn push_elements(
    lines: &mut Vec<String>,
    title: &str,
    elements: &[words_to_data::document::NodeData],
) {
    if elements.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push(format!("**{title}**:"));
    for elem in elements {
        lines.push(format!("- {}:", elem.path));
        for field in [
            TextContentField::Chapeau,
            TextContentField::Heading,
            TextContentField::Proviso,
            TextContentField::Content,
            TextContentField::Continuation,
        ] {
            if let Some(val) = elem.get_text_content(field) {
                lines.push(format!(" * {}: {val}", field_name(&field)));
            }
        }
    }
}

fn trimmed_or_empty(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        "(empty)".to_string()
    } else {
        t.to_string()
    }
}

/// Lowercase display name for a text content field.
fn field_name(field: &TextContentField) -> &'static str {
    match field {
        TextContentField::Heading => "heading",
        TextContentField::Chapeau => "chapeau",
        TextContentField::Proviso => "proviso",
        TextContentField::Content => "content",
        TextContentField::Continuation => "continuation",
    }
}

/// Read the action a model named, and say so when it cannot be read.
///
/// The prompt asks for an operation and a model may answer with a word that is
/// neither the publisher's nor a drafter's. The caller then has to fall back, and
/// the fallback is only honest if the word it replaced was named first.
fn read_operation(operation: &str) -> Option<AmendingAction> {
    match AmendingAction::from_prose(operation) {
        Ok(action) => Some(action),
        Err(e) => {
            eprintln!(
                "warning: {e}, so the annotation records `amend` instead of what the model said"
            );
            None
        }
    }
}

/// Snake-case string for an amending action (matches its serde representation).
fn action_str(action: &AmendingAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Build a path to `filename` in the same directory as `dataset_path`.
fn sibling(dataset_path: &str, filename: &str) -> PathBuf {
    Path::new(dataset_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}

/// Serialize the candidate view to pretty JSON for inspection.
fn candidates_view(matches: &[AmendmentMatch]) -> Vec<serde_json::Value> {
    matches
        .iter()
        .map(|m| {
            serde_json::json!({
                "bill_id": m.bill_id,
                "amendment_id": m.amendment_id,
                "amending_text": m.amending_text,
                "candidates": m.candidates.iter().map(|c| serde_json::json!({
                    "diff": c.diff,
                    "similarity": c.similarity,
                    "mentions": c.mentions,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// System prompt (ported verbatim from the Python `ai_bill_matching` tool).
const SYSTEM_PROMPT: &str = r#"You are an expert legislative analyst specializing in matching bill amendments to changes in the US Code.

## Your Task
Given a bill amendment and a list of candidate diffs (changes between two versions of US Code), identify which candidate(s) correspond to the amendment.

## Amendment Structure
- **action_types**: The type of change (amend, insert, strike, repeal)
- **amending_text**: The legislative language describing the change, typically including:
  - Target section reference (e.g., "Section 36B(e)(1)")
  - Action to take (e.g., "is amended by inserting", "is amended by striking")
  - The specific text changes

## Candidate Structure
Each candidate is formatted with:

**Location**: Hierarchical path to the code section (e.g., "Section 174 (a) (2) (B)")

**Similarity**: Pre-computed text similarity metrics
- `score`: Overall similarity (0.0-1.0)
- `precision`: Fraction of diff words found in amendment
- `recall`: Fraction of amendment words found in diff

**Mentions**: Section references from the amendment text found at this location

**Changes**: Text modifications, showing for each changed field:
- `field_name`: Which field changed (heading, chapeau, content, proviso, continuation)
- `old`: Original text
- `new`: Updated text

**Added Elements**: New child elements, each with:
- `path`: Location path
- Text content fields (heading, chapeau, content, proviso, continuation)

**Removed Elements**: Deleted child elements (same structure as added)

## Matching Strategy

1. **Parse the section reference** from amendment text (e.g., "Section 36B(e)(1)")
   - Match against candidate `Location` path and `Mentions`

2. **Verify action type alignment**:
   - "insert" → additions in `changes` or `added`
   - "strike" → deletions in `changes` or `removed`
   - "amend" → modifications with both old and new values
   - "repeal" → heading shows "Repealed" or section removed

3. **Use similarity scores as strong signals**:
   - score > 0.9 with high recall → very likely match
   - score > 0.7 → probable match, verify content
   - score < 0.5 → unlikely unless mentions are strong

4. **Verify content alignment**: The specific text changes should match

5. **Allow multiple matches** for cross-references or conforming amendments

## Output Format

Return valid JSON with `annotations` array:
```json
{
  "annotations": [
    {
      "candidate_index": 0,
      "operation": "amend",
      "causative_text": "by striking '$100' and inserting '$200'",
      "confidence": 0.95,
      "reasoning": "Section reference matches uslm_id, similarity score 0.97, and the inserted text aligns with diff changes"
    }
  ],
  "no_match_reasoning": null
}
```

- `candidate_index`: Index of the matching candidate (0-based)
- `operation`: The legal operation type. These are the publisher's own words, from
  `AmendingActionTypeEnum` in the USLM schema, and the answer must be one of them:
  - "enact" - Enacting a law
  - "add" - Adding a provision to existing law
  - "amend" - Modifying an existing provision
  - "substitute" - Replacing a provision, including "striking X and inserting Y"
  - "redesignate" - Renumbering a provision that stays where it is
  - "repeal" - Repealing a provision
  - "repealAndReserve" - Repealing a provision and reserving its place
  - "insert" - Adding text to a provision
  - "delete" - Removing text from a provision, that is, striking it
  - "conform" - Making text the same as defined replacement text
  - "noChange" - No change is directed
  - "unknown" - An action none of the above describes
  There is no action for relocation. A provision moved to another title is
  "redesignate" only if it was renumbered; otherwise say "unknown" and explain it
  in `reasoning`.
- `causative_text`: **IMPORTANT** - This must be an EXACT substring copied from the amending_text that specifically causes THIS change. Extract only the relevant portion, NOT the entire amending_text. Examples:
  - If amending_text is "Section 123(a) is amended by striking '$100' and inserting '$200', and by adding at the end the following new paragraph..."
  - For the strike/insert change: `"by striking '$100' and inserting '$200'"`
  - For the new paragraph: `"by adding at the end the following new paragraph"`
  - Do NOT use the full amending_text, unless it truly matches the entirety of the change
- `confidence`: Float from 0.0 to 1.0 (e.g., 0.95 for high confidence, 0.7 for medium, 0.4 for low)
- `reasoning`: Brief explanation of why this candidate matches
- Include `no_match_reasoning` only when annotations is empty

## Important Notes
- An amendment CAN match multiple candidates
- An amendment may match ZERO candidates if the change isn't in the provided diffs
- Trust high similarity scores but verify section references
- Parse legislative language carefully (e.g., "paragraph (2)(A)" = subparagraph A of paragraph 2)
"#;

/// Build the request options from the CLI flags.
fn chat_options(
    params: &[String],
    temperature: f32,
    max_tokens: Option<u64>,
) -> Result<ChatOptions, String> {
    let mut extra = serde_json::Map::new();
    for param in params {
        let (key, value) = words_to_data::llm::parse_param(param)?;
        extra.insert(key, value);
    }
    Ok(ChatOptions {
        temperature,
        max_tokens,
        extra,
    })
}
