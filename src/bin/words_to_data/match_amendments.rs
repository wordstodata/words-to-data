//! `words_to_data match-amendments` — match bill amendments to US Code changes.
//!
//! For each amendment we gather candidate diffs (via pre-computed similarity
//! scores + section-mention scans), ask an LLM which candidate(s) the amendment
//! actually caused, and record the answer as a `ChangeAnnotation` written back
//! into the dataset in place.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Args as ClapArgs;
use serde::{Deserialize, Serialize};
use words_to_data::annotation::{
    AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation,
};
use words_to_data::dataset::{Dataset, Format};
use words_to_data::legislature::AmendingAction;
use words_to_data::matching::{
    AmendmentMatch, Candidate, DEFAULT_SIMILARITY_CUTOFF, build_matches,
};
use words_to_data::uslm::TextContentField;

use crate::llm::{ChatOptions, LlmClient};
use crate::span::Span;

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

    /// Number of concurrent LLM requests
    #[arg(long, default_value_t = 1)]
    pub threads: usize,

    /// Only offer the model candidates scoring strictly above this cutoff
    #[arg(long, default_value_t = DEFAULT_SIMILARITY_CUTOFF)]
    pub similarity_cutoff: f32,

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

pub fn run(args: Args) {
    let mut dataset = crate::fail::or_exit(
        Dataset::load(&args.dataset, Format::Compact),
        "Error loading dataset",
    );

    let llm = LlmClient::new(args.base_url.clone(), args.model.clone(), None);
    let annotator = format!(
        "model:{}",
        if args.model.is_empty() {
            "local"
        } else {
            &args.model
        }
    );

    let pairs = args.span.resolve(&dataset);
    let mut candidates_by_work = Vec::new();
    let mut applied = 0;
    let mut annotated_paths = 0;

    for (from, to) in pairs {
        let diff = crate::fail::or_exit(dataset.compute_diff(&from, &to), "Error computing diff");

        let matches = build_matches(&dataset, &diff, args.similarity_cutoff);
        println!("\n{from} -> {to}");
        print_stats(&matches);

        // Ask the LLM which candidate(s) each amendment matches.
        let matched = classify_all(&llm, &matches, args.threads);

        // Apply the LLM's annotations single-threaded.
        for (match_idx, annotations) in matched {
            let m = &matches[match_idx];
            for ann in annotations {
                let Some(candidate) = usize::try_from(ann.candidate_index)
                    .ok()
                    .and_then(|i| m.candidates.get(i))
                else {
                    continue;
                };

                let annotation = ChangeAnnotation {
                    operation: ann
                        .operation
                        .as_deref()
                        .and_then(|op| AmendingAction::from_str(op).ok())
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
                        annotator: annotator.clone(),
                        timestamp: time::OffsetDateTime::now_utc(),
                        notes: None,
                        reasoning: ann.reasoning,
                    },
                };
                crate::fail::or_exit(
                    dataset.add_annotation(&from, &to, annotation),
                    "Error adding annotation",
                );
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

    let output = args.output.as_deref().unwrap_or(&args.dataset);
    crate::fail::or_exit(
        dataset.save(output, Format::Compact),
        "Error saving dataset",
    );

    println!(
        "\nApplied {applied} annotation(s) across {} work(s)",
        candidates_by_work.len()
    );
    println!("Annotated paths: {annotated_paths}");
    println!("Wrote {}", candidates_path.display());
    println!("Wrote {output}");
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

/// Run LLM classification over every match using `threads` OS worker threads.
///
/// Workers only read match data and return `(match_index, annotations)`; the
/// caller applies the annotations to the dataset single-threaded.
fn classify_all(
    llm: &LlmClient,
    matches: &[AmendmentMatch],
    threads: usize,
) -> Vec<(usize, Vec<LlmAnnotation>)> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Vec<LlmAnnotation>)>> = Mutex::new(Vec::new());
    let worker_count = threads.max(1);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(m) = matches.get(i) else { break };

                    match classify(llm, m) {
                        Ok(annotations) => {
                            results.lock().unwrap().push((i, annotations));
                        }
                        Err(err) => {
                            eprintln!("ERROR matching {}: {err}", m.amendment_id);
                        }
                    }

                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    println!("[{n}/{}] matched", matches.len());
                }
            });
        }
    });

    results.into_inner().unwrap()
}

/// Query the LLM for one amendment and parse its annotation list.
fn classify(llm: &LlmClient, m: &AmendmentMatch) -> Result<Vec<LlmAnnotation>, String> {
    let user_prompt = build_user_prompt(m);
    let opts = ChatOptions {
        temperature: 0.0,
        max_tokens: None,
    };
    let raw = llm.chat(SYSTEM_PROMPT, &user_prompt, &opts)?;
    parse_response(&raw).map_err(|e| format!("{e}\n--- raw model output ---\n{raw}"))
}

/// The LLM's JSON response envelope.
#[derive(Deserialize)]
struct LlmResponse {
    #[serde(default)]
    annotations: Vec<LlmAnnotation>,
}

/// One annotation the LLM proposes for an amendment.
#[derive(Deserialize)]
struct LlmAnnotation {
    /// Index into the match's candidate list (negative means "no match").
    #[serde(default = "neg_one")]
    candidate_index: i64,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    causative_text: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    reasoning: Option<String>,
}

fn neg_one() -> i64 {
    -1
}

/// Parse the model's JSON response, tolerating a ```json fenced block.
fn parse_response(raw: &str) -> Result<Vec<LlmAnnotation>, String> {
    let mut text = raw.trim();
    if let Some(stripped) = text.strip_prefix("```") {
        // Drop the opening fence line (e.g. "```json") and the closing fence.
        let after_lang = stripped
            .find('\n')
            .map(|n| &stripped[n + 1..])
            .unwrap_or("");
        text = after_lang.strip_suffix("```").unwrap_or(after_lang).trim();
    }
    let response: LlmResponse = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(response.annotations)
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
    elements: &[words_to_data::uslm::ElementData],
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
- `operation`: The legal operation type. Must be one of:
  - "amend" - Modifying existing text
  - "add" - Adding new content
  - "delete" - Removing content
  - "insert" - Inserting new elements
  - "redesignate" - Renumbering or renaming sections
  - "repeal" - Repealing a section entirely
  - "move" - Moving content to a different location
  - "strike" - Striking text
  - "strikeandinsert" - Striking and replacing with new text
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
