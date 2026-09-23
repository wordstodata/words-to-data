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
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use words_to_data::dataset::{Dataset, DatasetMetadata, Format, WorkId};
use words_to_data::link::LinkKind;
use words_to_data::storage::{EvidenceReader, LinkReader};
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

/// The longest real recorded reply for `command`, as the model sent it.
fn real_reply(command: &str) -> String {
    #[derive(serde::Deserialize)]
    struct RecordedReply {
        command: String,
        reply: String,
    }
    let json = std::fs::read_to_string(REPLIES).expect("the fixture should be readable");
    let replies: Vec<RecordedReply> =
        serde_json::from_str(&json).expect("the fixture should parse");
    replies
        .into_iter()
        .filter(|r| r.command == command)
        .max_by_key(|r| r.reply.len())
        .expect("the fixture should hold a reply for this command")
        .reply
}

/// A real recorded reply for `command`, cut in half.
///
/// A reply that failed to parse is never stored (`docs/adr/0005`), so the corpus
/// holds no malformed reply and never will. Cutting a real one short is not
/// invented data: it is what a `max_tokens` ceiling does to a reply in flight,
/// and the surviving half is the model's own text.
fn truncated_real_reply(command: &str) -> String {
    let full = real_reply(command);
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
    counting_stub_server(content).0
}

/// The same stub, with a count of the replies it has served.
///
/// A cache is only real if a second run calls no model, and the count is the
/// only thing that can say so: the dataset looks the same either way.
fn counting_stub_server(content: String) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the stub should bind");
    let address = listener
        .local_addr()
        .expect("the stub should have an address");

    let body = serde_json::json!({
        "choices": [{ "message": { "content": content } }]
    })
    .to_string();

    let calls = Arc::new(AtomicUsize::new(0));
    let served = Arc::clone(&calls);

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
            served.fetch_add(1, Ordering::SeqCst);
        }
    });

    (format!("http://{address}"), calls)
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
    let (dataset, directory) = matching_fixture(name);
    let path = format!("{directory}/dataset.json");
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");
    path
}

/// The same fixture as a database, which is the form the pipeline is meant to
/// work against (#195).
fn sqlite_dataset_for_matching(name: &str) -> String {
    let (dataset, directory) = matching_fixture(name);
    let path = format!("{directory}/dataset.sqlite");
    // A database left by an earlier run would already hold this run's links.
    let _ = std::fs::remove_file(&path);
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// Build the fixture in memory, in its own directory, and hand back both.
fn matching_fixture(name: &str) -> (Dataset<words_to_data::storage::InMemoryStorage>, String) {
    let directory = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&directory).expect("the fixture directory should exist");
    // A cache left by an earlier run of this test would answer for the server.
    let _ = std::fs::remove_file(format!("{directory}/matches_cache.json"));

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

    (dataset, directory)
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

    let output = run_match_amendments(&dataset_path, &base_url, &[]);

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

/// Where a run over a W2D file puts its result, beside the input rather than
/// over it (#186). A database is changed where it sits and names nowhere.
fn output_beside(dataset_path: &str) -> Option<String> {
    (!dataset_path.ends_with(".sqlite")).then(|| {
        std::path::Path::new(dataset_path)
            .with_file_name("annotated.json")
            .to_string_lossy()
            .into_owned()
    })
}

/// Run `match-amendments` over the pair of release points the fixture holds.
fn run_match_amendments(
    dataset_path: &str,
    base_url: &str,
    extra: &[&str],
) -> std::process::Output {
    let from = format!("{TITLE_26}@{EARLY}");
    let to = format!("{TITLE_26}@{LATE}");
    let mut command = Command::new(env!("CARGO_BIN_EXE_words_to_data"));
    command.args([
        "match-amendments",
        dataset_path,
        "--from",
        &from,
        "--to",
        &to,
        "--base-url",
        base_url,
        "--threads",
        "1",
    ]);
    if let Some(output) = output_beside(dataset_path) {
        command.args(["--output", &output]);
    }
    command.args(extra).output().expect("the binary should run")
}

/// A W2D file is written whole, so a run that wrote back over its input would
/// destroy the dataset if it stopped part way (#186). A match run holds
/// hundreds of model calls that cost money to make again, so it asks where the
/// result goes before it spends the first one (#195).
#[test]
fn should_refuse_to_write_over_its_input_when_a_w2d_file_names_no_output() {
    let dataset_path = dataset_for_matching("llm_match_no_output");
    let (base_url, calls) = counting_stub_server(real_reply("match-amendments"));

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
        !output.status.success(),
        "the command should refuse to write back over a W2D file"
    );

    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(
        complaint.contains("will not write back over"),
        "it should say it will not write over the input, got: {complaint}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a run with nowhere to put its result should buy no reply"
    );
}

/// A database is changed where it sits, so the run needs nowhere to write it
/// (#195). `match-amendments` used to refuse a SQLite dataset and say to
/// convert it to JSON first, which is the form that cannot give the run a
/// transaction.
#[test]
fn should_change_the_database_in_place_when_match_amendments_is_given_sqlite() {
    let dataset_path = sqlite_dataset_for_matching("llm_match_sqlite");
    let base_url = start_stub_server(real_reply("match-amendments"));

    // No --output: the database is where the result belongs.
    let output = run_match_amendments(&dataset_path, &base_url, &[]);

    assert!(
        output.status.success(),
        "the command should accept a database, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let changed = Dataset::open_sqlite(&dataset_path).expect("the database should open");
    let links = changed
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("reading the links should work");
    assert!(
        !links.is_empty(),
        "the run should leave its links in the database it was given"
    );
}

/// The matching method was chosen arbitrarily and may be replaced, so a link it
/// made must say which method made it and which version that method was at
/// (#182, #179 decision 10). Without the version, a replacement is a silent
/// change of meaning across a dataset that reads the same.
///
/// The run is recorded beside the links, so the dataset can say that this
/// reasoning was applied to this window (decision 11).
#[test]
fn should_name_the_matching_method_and_its_version_when_match_amendments_writes_links() {
    let dataset_path = sqlite_dataset_for_matching("llm_match_method");
    let base_url = start_stub_server(real_reply("match-amendments"));

    let output = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        output.status.success(),
        "the run should finish, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let changed = Dataset::open_sqlite(&dataset_path).expect("the database should open");
    let links = changed
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("reading the links should work");
    assert!(!links.is_empty(), "the run should leave links behind");

    let expected = words_to_data::matching::matching_method();
    for link in &links {
        assert_eq!(
            link.provenance.method.as_ref(),
            Some(&expected),
            "a matched link should name the method that made it"
        );
    }

    let runs = changed.method_runs();
    assert_eq!(runs.len(), 1, "one method ran over one window, got {runs:?}");
    assert_eq!(runs[0].method, expected);
    assert!(
        runs[0].covers(&WorkId::new(TITLE_26), EARLY, LATE),
        "the record should name the window the run covered, got {:?}",
        runs[0]
    );
}

/// Every call `match-amendments` makes is bought, so a second run over an
/// unchanged dataset must buy nothing (#123). The dataset reads the same
/// whether a reply was cached or re-queried, so the test counts the calls the
/// server answered, and reads the count the run reports.
#[test]
fn should_make_no_model_call_when_match_amendments_runs_again_over_an_unchanged_dataset() {
    let dataset_path = dataset_for_matching("llm_match_cache");
    let (base_url, calls) = counting_stub_server(real_reply("match-amendments"));

    let first = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        first.status.success(),
        "the first run should exit zero, stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let bought = calls.load(Ordering::SeqCst);
    assert!(bought > 0, "the first run should call the model");

    let second = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        second.status.success(),
        "the second run should exit zero, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        bought,
        "the second run should call the model for nothing"
    );

    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains(&format!("{bought} replies reused")),
        "the run should say how many replies it reused, got:\n{stdout}"
    );
}

/// A prompt hash this build does not produce, in the shape of a real one.
const ANOTHER_PROMPT: &str = "00000000000000000000000000000000000000000000000000000000000000ff";

/// The reply cache `match-amendments` keeps beside a dataset.
fn matches_cache_path(dataset_path: &str) -> std::path::PathBuf {
    std::path::Path::new(dataset_path).with_file_name("matches_cache.json")
}

/// Say that every cached reply answered a prompt this build does not send.
///
/// The candidates are left alone, so the cache still holds a reply for each of
/// them: only the prompt behind those replies is now another one.
fn say_the_cached_replies_answered_another_prompt(dataset_path: &str) {
    let path = matches_cache_path(dataset_path);
    let text = std::fs::read_to_string(&path).expect("the first run should write a cache");
    let mut cache: serde_json::Value = serde_json::from_str(&text).expect("the cache should parse");
    let entries = cache
        .as_object_mut()
        .expect("the cache should be an object of cached replies");
    assert!(
        !entries.is_empty(),
        "the first run should cache its replies"
    );

    for entry in entries.values_mut() {
        let prompt_hash = entry
            .get_mut("prompt_hash")
            .expect("a cached reply should record the prompt it answered");
        *prompt_hash = serde_json::Value::String(ANOTHER_PROMPT.to_string());
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&cache).expect("the cache should serialize"),
    )
    .expect("the cache should be writable");
}

/// A reply answers a question, and the prompt is half of that question. A cache
/// that looked at the candidates alone would hand back a reply to a prompt this
/// build no longer sends, and the dataset would show nothing of it (#123).
///
/// The prompt cannot be changed from the command line, so the test changes the
/// cache the way a changed prompt does: the candidates stay, and the prompt
/// behind each stored reply is one this build does not produce.
#[test]
fn should_query_the_model_again_when_the_prompt_behind_a_cached_reply_has_changed() {
    let dataset_path = dataset_for_matching("llm_match_prompt_change");
    let (base_url, calls) = counting_stub_server(real_reply("match-amendments"));

    let first = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        first.status.success(),
        "the first run should exit zero, stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let bought = calls.load(Ordering::SeqCst);
    assert!(bought > 0, "the first run should fill the cache");

    say_the_cached_replies_answered_another_prompt(&dataset_path);

    let second = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        second.status.success(),
        "the second run should exit zero, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        bought * 2,
        "a reply bought under another prompt answers nothing, so every amendment should be asked again"
    );

    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("0 replies reused"),
        "no stale reply should be reused, got:\n{stdout}"
    );
}

/// `extract-changes` has `--no-cache` for the run that has to buy its answers
/// again. `match-amendments` answers to the same flag (#123).
#[test]
fn should_query_the_model_again_when_no_cache_is_given() {
    let dataset_path = dataset_for_matching("llm_match_no_cache");
    let (base_url, calls) = counting_stub_server(real_reply("match-amendments"));

    let first = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        first.status.success(),
        "the first run should exit zero, stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let bought = calls.load(Ordering::SeqCst);
    assert!(bought > 0, "the first run should fill the cache");

    let second = run_match_amendments(&dataset_path, &base_url, &["--no-cache"]);
    assert!(
        second.status.success(),
        "the second run should exit zero, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        bought * 2,
        "the flag should send every amendment to the model again"
    );

    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("0 replies reused"),
        "the flag should reuse nothing, got:\n{stdout}"
    );
}

/// Start `match-amendments` and leave it running.
///
/// Its output goes nowhere: a killed run's progress lines are not what the
/// caller reads, and a pipe nobody empties would stop the run before the kill.
fn spawn_match_amendments(dataset_path: &str, base_url: &str) -> std::process::Child {
    let from = format!("{TITLE_26}@{EARLY}");
    let to = format!("{TITLE_26}@{LATE}");
    let output = output_beside(dataset_path).expect("a W2D fixture names an output");
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "match-amendments",
            dataset_path,
            "--from",
            &from,
            "--to",
            &to,
            "--base-url",
            base_url,
            "--threads",
            "1",
            "--output",
            &output,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the binary should run")
}

/// How many replies the cache beside the dataset holds, or none when the run
/// has not written it yet.
fn cached_reply_count(dataset_path: &str) -> usize {
    let Ok(text) = std::fs::read_to_string(matches_cache_path(dataset_path)) else {
        return 0;
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|cache| cache.as_object().map(serde_json::Map::len))
        .unwrap_or(0)
}

/// Wait until the running command has cached at least `count` replies.
fn wait_for_cached_replies(dataset_path: &str, count: usize) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    while std::time::Instant::now() < deadline {
        if cached_reply_count(dataset_path) >= count {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the run should cache {count} replies before the deadline");
}

/// The number a run printed after `label`, for example the total it matched.
fn number_after(stdout: &str, label: &str) -> usize {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(label))
        .expect("the run should print this line")
        .trim()
        .parse()
        .expect("the line should end in a number")
}

/// A sweep over a corpus runs for hours, and the machine it runs on does not
/// always wait. What a killed run bought is kept, so the next run pays only for
/// what is left (#123).
#[test]
fn should_resume_from_the_cache_when_a_run_is_interrupted() {
    let dataset_path = dataset_for_matching("llm_match_resume");
    let (base_url, calls) = counting_stub_server(real_reply("match-amendments"));

    // Kill the first run once it has bought a few replies.
    let mut run = spawn_match_amendments(&dataset_path, &base_url);
    wait_for_cached_replies(&dataset_path, 5);
    run.kill().expect("the run should stop");
    run.wait().expect("the run should end");

    let kept = cached_reply_count(&dataset_path);
    assert!(kept >= 5, "the killed run should leave what it bought");
    let bought = calls.load(Ordering::SeqCst);

    let second = run_match_amendments(&dataset_path, &base_url, &[]);
    assert!(
        second.status.success(),
        "the second run should exit zero, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains(&format!("{kept} replies reused")),
        "the second run should reuse every reply the killed one bought, got:\n{stdout}"
    );

    let total = number_after(&stdout, "Total amendments with candidates:");
    assert!(
        kept < total,
        "the first run should have been stopped part-way, with {total} to buy and {kept} bought"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst) - bought,
        total - kept,
        "the second run should buy what is left, and nothing more"
    );
}
