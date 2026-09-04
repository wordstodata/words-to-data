//! End-to-end test for the LLM-bound half of the pipeline.
//!
//! `extract-changes` talks to an OpenAI-compatible server. This test stands a
//! stub server in front of it and drives the real binary, so the client, the
//! retry wrapper, the `<response>` parsing, and the dataset write all run for
//! real. Only the network peer is fake, which is the system boundary that
//! `CLAUDE.md` allows mocking.
//!
//! The reply content is real: it comes from `changes_cache.json`, the surviving
//! output of a production extraction run. The wrapper around it is
//! reconstructed, because the raw model replies were never persisted (#58). So
//! this proves the pipeline works. It does not prove the parser survives the
//! formatting a model really emits, and it cannot until #58 is done.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::uslm::bill_parser::parse_bill_amendments;

/// A real public law from the test corpus: HR 1 of the 119th Congress.
const BILL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-21";

/// One real extraction result, lifted verbatim from `changes_cache.json`.
const REAL_EXTRACTION: &str =
    r#"[{"added":["of—\"(A)"],"removed":["of"]},{"added":[";"],"removed":["."]}]"#;

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
    let directory = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&directory).expect("the fixture directory should exist");
    // A stale sibling cache would let the command skip the server entirely.
    let _ = std::fs::remove_file(format!("{directory}/changes_cache.json"));

    let mut bill = parse_bill_amendments(BILL_ID, BILL_XML).expect("the public law should parse");

    let mut ids: Vec<String> = bill.amendments.keys().cloned().collect();
    ids.sort();
    let keep = ids
        .first()
        .expect("the bill should have amendments")
        .clone();
    bill.amendments.retain(|id, _| *id == keep);

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "LLM Test Fixture".to_string(),
        description: "One real amendment from HR 1".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    });
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
