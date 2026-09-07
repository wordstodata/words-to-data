//! The chat client against a stub server.
//!
//! An HTTP endpoint is a system boundary, which is where `CLAUDE.md` allows a
//! test to stand something in. Everything below runs against a real socket on
//! loopback, so the request that goes out is the request under test.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

use words_to_data::llm::{ChatOptions, LlmClient};

/// One recorded request: its headers, so a test can assert what was sent.
struct Recorded {
    headers: Vec<String>,
}

/// Serve `count` requests with the given status and body, recording each.
///
/// Returns the base URL and a receiver of what arrived.
fn stub_server(
    count: usize,
    status: u16,
    body: &'static str,
) -> (String, mpsc::Receiver<Recorded>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("local addr").port();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        for _ in 0..count {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            serve_one(stream, status, body, &tx);
        }
    });

    (format!("http://127.0.0.1:{port}"), rx)
}

fn serve_one(mut stream: TcpStream, status: u16, body: &str, tx: &mpsc::Sender<Recorded>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end().to_string();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
        headers.push(line);
    }
    // Drain the body so the client's write completes before we answer.
    let mut body_buf = vec![0u8; content_length];
    let _ = reader.read_exact(&mut body_buf);

    let _ = tx.send(Recorded { headers });

    let reason = if status == 200 { "OK" } else { "Error" };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

const OK_BODY: &str = r#"{"choices":[{"message":{"content":"hello"}}]}"#;

#[test]
fn should_send_the_api_key_as_a_bearer_token_when_one_is_given() {
    let (base_url, requests) = stub_server(1, 200, OK_BODY);
    let client = LlmClient::new(
        base_url,
        "deepseek-chat".to_string(),
        Some("sk-secret".into()),
    );

    let answer = client
        .chat("system", "user", &ChatOptions::default())
        .expect("the stub answers");

    assert_eq!(answer, "hello");
    let recorded = requests.recv().expect("a request should have arrived");
    assert!(
        recorded
            .headers
            .iter()
            .any(|h| h.eq_ignore_ascii_case("Authorization: Bearer sk-secret")),
        "a hosted endpoint needs the key on the request, got {:?}",
        recorded.headers
    );
}

#[test]
fn should_send_no_authorization_header_when_no_key_is_given() {
    let (base_url, requests) = stub_server(1, 200, OK_BODY);
    let client = LlmClient::new(base_url, "local".to_string(), None);

    client
        .chat("system", "user", &ChatOptions::default())
        .expect("the stub answers");

    // A local llama.cpp server needs no key, and sending an empty bearer would
    // be worse than sending nothing.
    let recorded = requests.recv().expect("a request should have arrived");
    assert!(
        !recorded
            .headers
            .iter()
            .any(|h| h.to_lowercase().starts_with("authorization:")),
        "got {:?}",
        recorded.headers
    );
}

#[test]
fn should_fail_immediately_when_the_key_is_rejected() {
    // Offer to serve five, so a retrying client would be answered rather than
    // blocked. If only one arrives, the client chose not to retry.
    let (base_url, requests) = stub_server(5, 401, r#"{"error":"bad key"}"#);
    let client = LlmClient::new(base_url, "deepseek-chat".to_string(), Some("wrong".into()));

    let started = std::time::Instant::now();
    let error = client
        .chat("system", "user", &ChatOptions::default())
        .expect_err("a rejected key is an error");

    // The backoff ladder is 5s, 10s, 20s, 40s. Retrying a 401 costs 75 seconds
    // and fails anyway, which reads as a hang rather than as a mistyped key.
    assert!(
        started.elapsed() < std::time::Duration::from_secs(4),
        "a rejected key should fail fast, took {:?}",
        started.elapsed()
    );
    assert!(
        error.contains("401") && error.contains("W2D_API_KEY"),
        "the error should name the cause and the fix, got: {error}"
    );

    assert!(requests.recv().is_ok(), "one request should have been sent");
    assert!(
        requests
            .recv_timeout(std::time::Duration::from_millis(300))
            .is_err(),
        "no second request should have been sent"
    );
}

#[test]
fn should_retry_when_the_server_asks_for_another_go() {
    // 429 is the server asking to be tried again, unlike a rejected key.
    let (base_url, requests) = stub_server(2, 429, r#"{"error":"slow down"}"#);
    let mut client = LlmClient::new(base_url, "deepseek-chat".to_string(), None);
    client.retry_after(std::time::Duration::from_millis(10));

    let _ = client.chat("system", "user", &ChatOptions::default());

    assert!(requests.recv().is_ok(), "the first attempt");
    assert!(
        requests
            .recv_timeout(std::time::Duration::from_secs(2))
            .is_ok(),
        "a rate limit should be retried"
    );
}
