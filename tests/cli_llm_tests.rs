//! End-to-end test for the LLM-bound half of the pipeline.
//!
//! `extract-changes` talks to an OpenAI-compatible server. This test stands a
//! stub server in front of it and drives the real binary, so the client, the
//! retry wrapper, the `<response>` parsing, and the dataset write all run for
//! real. Only the network peer is fake, which is the system boundary that
//! `CLAUDE.md` allows mocking.
//!
//! The reply content is real. The well-formed cases come from a production run:
//! either `changes_cache.json` or the recorded replies in the corpus. The
//! malformed cases are a recorded reply cut short, which is what a `max_tokens`
//! ceiling does to a reply in flight.
//!
//! Cutting one short is the only way to get a malformed fixture, and it stays
//! that way: a reply that does not parse produced no statement, so it is not
//! evidence and is never recorded
//! (`docs/adr/0005-evidence-is-stored-once-and-never-deleted.md`). No sweep will
//! ever add one to the corpus. What a run does instead is report the loss, and
//! that is what the failure tests below pin.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::storage::EvidenceReader;
use words_to_data::uslm::bill_parser::parse_bill_amendments;

/// A real public law from the test corpus: HR 1 of the 119th Congress.
const BILL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-21";

/// The title HR 1 amends, at the two release points the corpus holds. Title 51
/// is smaller, but it did not change between them and HR 1 does not mention it,
/// so it yields no candidates and the model is never called.
const TITLE_26: &str = "uscode/title_26";
const USC26_EARLY: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const USC26_LATE: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// One real extraction result, lifted verbatim from `changes_cache.json`.
const REAL_EXTRACTION: &str =
    r#"[{"added":["of—\"(A)"],"removed":["of"]},{"added":[";"],"removed":["."]}]"#;

/// Real recorded replies, one per command shape.
const REPLIES: &str = "tests/test_data/processed/model_replies.json";

/// A real recorded reply for `command`, cut in half.
///
/// A reply that failed to parse is never stored (`docs/adr/0005`), so the corpus
/// holds no malformed reply and never will. Cutting a real one short is not
/// invented data: it is what a `max_tokens` ceiling does to a reply in flight,
/// and the surviving half is the model's own text.
fn truncated_real_reply(command: &str) -> String {
    #[derive(serde::Deserialize)]
    struct RecordedReply {
        command: String,
        reply: String,
    }
    let json = std::fs::read_to_string(REPLIES).expect("the fixture should be readable");
    let replies: Vec<RecordedReply> =
        serde_json::from_str(&json).expect("the fixture should parse");
    let full = replies
        .into_iter()
        .filter(|r| r.command == command)
        .max_by_key(|r| r.reply.len())
        .expect("the fixture should hold a reply for this command")
        .reply;
    full[..full.len() / 2].to_string()
}

/// The id of the single amendment in a fixture dataset.
fn only_amendment_id(dataset_path: &str) -> String {
    let dataset = Dataset::load(dataset_path, Format::Compact).expect("the fixture should load");
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should be there");
    bill.amendments
        .values()
        .next()
        .expect("the fixture should hold one amendment")
        .id
        .clone()
}

/// Serve one canned chat-completion reply to every request, forever.
///
/// Returns the base URL to point the CLI at. The thread is detached: it dies
/// with the test binary.
fn start_stub_server(content: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the stub should bind");
    let address = listener
        .local_addr()
        .expect("the stub should have an address");

    let body = serde_json::json!({
        "choices": [{ "message": { "content": content } }]
    })
    .to_string();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };

            // Read the request head, then its body, so the client sees a clean
            // exchange rather than a reset connection.
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                match stream.read(&mut byte) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => request.push(byte[0]),
                }
            }
            let head = String::from_utf8_lossy(&request).to_lowercase();
            let length: usize = head
                .split("content-length:")
                .nth(1)
                .and_then(|rest| rest.split("\r\n").next())
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(0);
            let mut discard = vec![0u8; length];
            let _ = stream.read_exact(&mut discard);

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    format!("http://{address}")
}

/// Write a compact-JSON dataset holding one real amendment, with no changes on
/// it yet. One amendment keeps the test to a single round trip; the amendment
/// itself is real, taken from the parsed public law.
fn dataset_with_one_amendment(name: &str) -> String {
    dataset_with_amendments(name, 1)
}

/// The same, holding the first `count` real amendments of the public law.
///
/// The bill carries 603, so a caller can ask for more than the summary will
/// name and still be working from real amendments.
fn dataset_with_amendments(name: &str, count: usize) -> String {
    let directory = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&directory).expect("the fixture directory should exist");
    // A stale sibling cache would let the command skip the server entirely.
    let _ = std::fs::remove_file(format!("{directory}/changes_cache.json"));

    let mut bill = parse_bill_amendments(BILL_ID, BILL_XML).expect("the public law should parse");

    let mut ids: Vec<String> = bill.amendments.keys().cloned().collect();
    ids.sort();
    let keep: std::collections::HashSet<String> = ids.into_iter().take(count).collect();
    assert_eq!(keep.len(), count, "the bill should have {count} amendments");
    bill.amendments.retain(|id, _| keep.contains(id));

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "LLM Test Fixture".to_string(),
        description: "One real amendment from HR 1".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    });
    dataset.add_bill(bill).expect("the bill should be added");

    let path = format!("{directory}/dataset.json");
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");
    path
}

/// A dataset `match-amendments` can work on: title 26 at both release points,
/// plus the public law that amends it, with word-level changes on the
/// amendments so the similarity channel produces candidates.
///
/// The changes are one amendment's real extraction, copied across the bill.
/// Extraction is an LLM's reading and the corpus holds no per-amendment result,
/// so this is the same compromise `matching_tests` makes, with real output
/// rather than invented words. It decides which candidates appear, not what the
/// command does with a reply, which is what the test is about.
fn dataset_for_matching(name: &str) -> String {
    let directory = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&directory).expect("the fixture directory should exist");

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Matching Test Fixture".to_string(),
        description: "Title 26 at two release points, with HR 1".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    });
    for (xml, date, label) in [(USC26_EARLY, EARLY, "Before"), (USC26_LATE, LATE, "After")] {
        dataset
            .add_uslm_xml(xml, date, Some(label.to_string()))
            .expect("the corpus should parse and load");
    }

    let changes =
        words_to_data::llm::parse_changes(&format!("<response>{REAL_EXTRACTION}</response>"))
            .expect("the real extraction should parse");
    let mut bill = parse_bill_amendments(BILL_ID, BILL_XML).expect("the public law should parse");
    for amendment in bill.amendments.values_mut() {
        amendment.changes = changes.clone();
    }
    dataset.add_bill(bill).expect("the bill should be added");

    let path = format!("{directory}/dataset.json");
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");
    path
}

#[test]
fn should_write_the_extracted_changes_into_the_dataset_when_extract_changes_runs() {
    let dataset_path = dataset_with_one_amendment("llm_extract");
    let base_url = start_stub_server(format!("<response>{REAL_EXTRACTION}</response>"));

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    assert!(
        output.status.success(),
        "extract-changes should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dataset = Dataset::load(&dataset_path, Format::Compact).expect("the result should load");
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should still be there");
    let amendment = bill
        .amendments
        .values()
        .next()
        .expect("the amendment should still be there");

    assert_eq!(
        amendment.changes.len(),
        2,
        "both changes in the reply should be written"
    );
    assert_eq!(amendment.changes[0].added, vec!["of—\"(A)".to_string()]);
    assert_eq!(amendment.changes[0].removed, vec!["of".to_string()]);
    assert_eq!(amendment.changes[1].added, vec![";".to_string()]);
    assert_eq!(amendment.changes[1].removed, vec![".".to_string()]);
}

/// The model often finds nothing: two of the first three amendments in the real
/// cache came back empty. An empty extraction must succeed and write nothing,
/// not fail and not invent a change.
#[test]
fn should_write_nothing_when_the_model_finds_no_changes() {
    let dataset_path = dataset_with_one_amendment("llm_empty");
    let base_url = start_stub_server("<response>[]</response>".to_string());

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    assert!(
        output.status.success(),
        "an empty extraction should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dataset = Dataset::load(&dataset_path, Format::Compact).expect("the result should load");
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should still be there");
    let amendment = bill
        .amendments
        .values()
        .next()
        .expect("the amendment should still be there");

    assert!(
        amendment.changes.is_empty(),
        "an empty reply must not invent changes, got {:?}",
        amendment.changes
    );
}

/// A reply that does not parse loses its amendment. The run has to say so, and
/// say which one, because the id is the key into the stderr record (#101).
#[test]
fn should_name_the_lost_amendment_when_a_reply_cannot_be_parsed() {
    let dataset_path = dataset_with_one_amendment("llm_unparseable");
    let amendment_id = only_amendment_id(&dataset_path);
    let base_url = start_stub_server(truncated_real_reply("extract-changes"));

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("1 amendment failed"),
        "the summary should count the loss, got:\n{stdout}"
    );
    assert!(
        stdout.contains(&amendment_id),
        "the summary should name the lost amendment {amendment_id}, got:\n{stdout}"
    );
}

/// The per-item progress line counted every reply as `extracted`, including the
/// ones that were lost, so the running total contradicted the summary (#101).
#[test]
fn should_not_count_a_failure_as_an_extraction_in_the_progress_line() {
    let dataset_path = dataset_with_one_amendment("llm_progress");
    let base_url = start_stub_server(truncated_real_reply("extract-changes"));

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("[1/1] extracted"),
        "a lost amendment must not read as extracted, got:\n{stdout}"
    );
    assert!(
        stdout.contains("[1/1] failed"),
        "the progress line should say the item failed, got:\n{stdout}"
    );
}

/// A sweep over a whole Congress can lose hundreds of amendments. Naming every
/// one buries the totals printed after it, so the list stops and says how many
/// it did not name (#101).
#[test]
fn should_cap_the_named_ids_when_more_amendments_fail_than_fit() {
    let dataset_path = dataset_with_amendments("llm_many_failures", 12);
    let base_url = start_stub_server(truncated_real_reply("extract-changes"));

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("12 amendments failed"),
        "the summary should count every loss, got:\n{stdout}"
    );
    assert!(
        stdout.contains("and 2 more"),
        "the summary should say how many it did not name, got:\n{stdout}"
    );

    // Ids are the only 64-character hex tokens the report prints.
    let named = stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.len() == 64 && line.chars().all(|c| c.is_ascii_hexdigit()))
        .count();
    assert_eq!(
        named, 10,
        "ten ids should be named before the count takes over, got:\n{stdout}"
    );
}

/// A reply that produced no statement is not evidence, so the dataset does not
/// carry it (`docs/adr/0005`). This pins that decision: the obvious way to
/// "improve" the report is to keep the text, and that is the rejected design.
#[test]
fn should_store_no_evidence_when_a_reply_cannot_be_parsed() {
    let dataset_path = dataset_with_one_amendment("llm_no_evidence");
    let base_url = start_stub_server(truncated_real_reply("extract-changes"));

    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    let dataset = Dataset::load(&dataset_path, Format::Compact).expect("the result should load");

    assert!(
        dataset
            .replies()
            .expect("replies should be readable")
            .is_empty(),
        "a reply that did not parse must not reach the reply store"
    );

    let bill = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should still be there");
    let amendment = bill
        .amendments
        .values()
        .next()
        .expect("the amendment should still be there");

    assert!(
        amendment.changes.is_empty(),
        "a lost amendment must carry no changes"
    );
    assert!(
        amendment.provenance.is_none(),
        "a lost amendment must carry no provenance, got {:?}",
        amendment.provenance
    );
}

/// Retrying is running the command again: a failure never enters the cache, so
/// the next run re-queries it with no flag (`docs/adr/0005`).
#[test]
fn should_retry_a_failed_amendment_on_the_next_run_without_a_flag() {
    let dataset_path = dataset_with_one_amendment("llm_retry");
    let failing = start_stub_server(truncated_real_reply("extract-changes"));

    let first = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &failing,
            "--threads",
            "1",
        ])
        .output()
        .expect("the binary should run");
    assert!(
        String::from_utf8_lossy(&first.stdout).contains("1 amendment failed"),
        "the first run should lose the amendment"
    );

    // Same command, no flags, against a server that now answers properly.
    let working = start_stub_server(format!("<response>{REAL_EXTRACTION}</response>"));
    let second = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &working,
            "--threads",
            "1",
        ])
        .output()
        .expect("the binary should run");

    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("1 to extract"),
        "the failed amendment should be queued again, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("failed"),
        "the retry should succeed, got:\n{stdout}"
    );

    let dataset = Dataset::load(&dataset_path, Format::Compact).expect("the result should load");
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should still be there");
    assert_eq!(
        bill.amendments
            .values()
            .next()
            .expect("the amendment should be there")
            .changes
            .len(),
        2,
        "the retry should write the changes the second reply carried"
    );
}

/// `match-amendments` loses an amendment to an unparseable reply exactly as
/// `extract-changes` does, and has to report it the same way (#101).
#[test]
fn should_name_the_lost_amendment_when_match_amendments_cannot_parse_a_reply() {
    let dataset_path = dataset_for_matching("llm_match_failure");
    let base_url = start_stub_server(truncated_real_reply("match-amendments"));

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "match-amendments",
            &dataset_path,
            "--from",
            &format!("{TITLE_26}@{EARLY}"),
            "--to",
            &format!("{TITLE_26}@{LATE}"),
            "--base-url",
            &base_url,
            "--threads",
            "1",
        ])
        .output()
        .expect("the binary should run");

    assert!(
        output.status.success(),
        "a lost amendment is not a failed run, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("amendments failed:") || stdout.contains("amendment failed:"),
        "the summary should report the loss, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("] matched"),
        "no reply parsed, so nothing should read as matched, got:\n{stdout}"
    );

    // The rejected design is to keep the text. Pin that it is not kept.
    let dataset = Dataset::load(&dataset_path, Format::Compact).expect("the result should load");
    assert!(
        dataset
            .replies()
            .expect("replies should be readable")
            .is_empty(),
        "a reply that did not parse must not reach the reply store"
    );
}

/// A partial failure is the normal state of an LLM sweep, not an error. A hard
/// exit would break a pipeline over three losses in nine hundred (#101).
#[test]
fn should_exit_zero_when_some_amendments_fail() {
    let dataset_path = dataset_with_one_amendment("llm_exit_code");
    let base_url = start_stub_server(truncated_real_reply("extract-changes"));

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "extract-changes",
            &dataset_path,
            "--base-url",
            &base_url,
            "--threads",
            "1",
            "--no-cache",
        ])
        .output()
        .expect("the binary should run");

    assert!(
        output.status.success(),
        "a lost amendment is not a failed run, got {:?}",
        output.status
    );
}
