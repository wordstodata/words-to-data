//! Minimal synchronous client for OpenAI-compatible chat-completions APIs.
//!
//! Targets a local `llama.cpp` server by default but works against any
//! `/v1/chat/completions` endpoint (DeepSeek, etc.). Deliberately blocking:
//! callers that want concurrency spawn OS threads and share `&LlmClient`,
//! which is `Send + Sync` because `ureq::Agent` is.
//!
//! It lives in the library rather than the binary so it can be tested against a
//! stub server. An HTTP endpoint is a system boundary, which is the one place
//! `CLAUDE.md` allows a test to stand something in.

use std::thread::sleep;
use std::time::Duration;

/// A blocking chat-completions client.
#[derive(Clone)]
pub struct LlmClient {
    agent: ureq::Agent,
    base_url: String,
    model: String,
    api_key: Option<String>,
    max_retries: u32,
    initial_backoff: Duration,
}

/// Per-request tuning knobs.
pub struct ChatOptions {
    pub temperature: f32,
    pub max_tokens: Option<u64>,
    /// Extra body fields, merged into the request as sent.
    ///
    /// Providers disagree about how to turn features on and off — reasoning
    /// effort, thinking budgets, sampling — and the names change faster than
    /// this client does. Rather than model each one and guess wrong, whatever
    /// is put here is sent verbatim, so a parameter this build has never heard
    /// of still reaches the endpoint.
    ///
    /// A key set here overrides the field of the same name above.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: None,
            extra: serde_json::Map::new(),
        }
    }
}

/// Parse a `key=value` request parameter.
///
/// The value is read as JSON so numbers, booleans, and objects survive, and
/// falls back to a plain string, which is what `reasoning_effort=low` should
/// mean without making the caller quote it.
pub fn parse_param(text: &str) -> Result<(String, serde_json::Value), String> {
    let (key, raw) = text
        .split_once('=')
        .ok_or_else(|| format!("expected key=value, got {text:?}"))?;
    if key.trim().is_empty() {
        return Err(format!("empty parameter name in {text:?}"));
    }
    let value =
        serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()));
    Ok((key.trim().to_string(), value))
}

impl LlmClient {
    /// Build a client. `api_key` is optional (local servers rarely need one);
    /// when present it is sent as a `Bearer` token.
    pub fn new(base_url: String, model: String, api_key: Option<String>) -> Self {
        Self {
            agent: ureq::Agent::new_with_defaults(),
            // Trim a trailing slash so we can join paths predictably.
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key,
            max_retries: 5,
            initial_backoff: Duration::from_secs(5),
        }
    }

    /// Shorten the backoff ladder.
    ///
    /// The default first wait is five seconds, which is right for a real
    /// endpoint and far too slow for a test that only wants to prove a retry
    /// happened.
    pub fn retry_after(&mut self, initial_backoff: Duration) -> &mut Self {
        self.initial_backoff = initial_backoff;
        self
    }

    /// Send a system+user prompt and return the assistant's message content.
    ///
    /// Retries transient failures with exponential backoff before giving up.
    pub fn chat(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        opts: &ChatOptions,
    ) -> Result<String, String> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
            "temperature": opts.temperature,
        });
        if let Some(max_tokens) = opts.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        // Merged last, so a caller can override anything set above.
        for (key, value) in &opts.extra {
            body[key.as_str()] = value.clone();
        }

        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut last_err = String::new();
        for attempt in 0..self.max_retries {
            match self.try_once(&url, &body) {
                Ok(content) => return Ok(content),
                Err((err, retryable)) => {
                    last_err = err;
                    if !retryable {
                        return Err(last_err);
                    }
                    if attempt + 1 < self.max_retries {
                        let backoff = self.initial_backoff * 2u32.pow(attempt);
                        eprintln!(
                            "LLM request failed (attempt {}/{}): {last_err}. Retrying in {}s...",
                            attempt + 1,
                            self.max_retries,
                            backoff.as_secs()
                        );
                        sleep(backoff);
                    }
                }
            }
        }
        Err(last_err)
    }

    /// Whether trying the same request again could plausibly succeed.
    ///
    /// A rejected key is not a transient failure. Retrying one costs the full
    /// backoff ladder — 75 seconds here — and then fails anyway, which reads as
    /// a hang rather than as the mistyped key it is.
    fn retryable(status: u16) -> bool {
        // 408 and 429 are the server asking for another go; 5xx is its problem,
        // not the request's. Every other 4xx is a fault in what we sent.
        status == 408 || status == 429 || status >= 500
    }

    /// A single request attempt (no retry).
    ///
    /// The `bool` says whether a retry is worth attempting.
    fn try_once(&self, url: &str, body: &serde_json::Value) -> Result<String, (String, bool)> {
        let mut request = self
            .agent
            .post(url)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", &format!("Bearer {key}"));
        }

        // Serialize/parse JSON ourselves so we don't need ureq's optional `json` feature.
        let payload = serde_json::to_vec(body).map_err(|e| (e.to_string(), false))?;
        let mut response = request.send(&payload).map_err(|err| match err {
            ureq::Error::StatusCode(status) => (describe_status(status), Self::retryable(status)),
            other => (other.to_string(), true),
        })?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| (e.to_string(), true))?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| (e.to_string(), false))?;

        value["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| (format!("Unexpected response shape: {value}"), false))
    }
}

/// Say what an HTTP status means for this request, so a rejected key does not
/// read as an unexplained failure.
fn describe_status(status: u16) -> String {
    match status {
        401 => {
            "401 Unauthorized: the server rejected the API key. Pass --api-key or set W2D_API_KEY."
                .to_string()
        }
        403 => "403 Forbidden: the API key is not allowed to use this model.".to_string(),
        404 => "404 Not Found: check --base-url; the path /v1/chat/completions is appended to it."
            .to_string(),
        429 => "429 Too Many Requests: rate limited.".to_string(),
        other => format!("HTTP status {other}"),
    }
}

/// The API key to use, from the flag or the environment.
///
/// An environment variable is the default because a key passed as a flag lands
/// in shell history and in `ps`. The flag stays for scripted runs that source
/// the key some other way.
pub fn api_key_from(flag: Option<&str>) -> Option<String> {
    flag.map(str::to_string)
        .or_else(|| std::env::var("W2D_API_KEY").ok())
        .filter(|key| !key.trim().is_empty())
}

/// One annotation the model proposes for an amendment.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LlmAnnotation {
    /// Index into the match's candidate list (negative means "no match").
    #[serde(default = "neg_one")]
    pub candidate_index: i64,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub causative_text: Option<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

fn neg_one() -> i64 {
    -1
}

/// The envelope the model wraps its annotations in.
///
/// Unknown fields are ignored on purpose: models add their own, such as a
/// `no_match_reasoning` beside an empty list, and a reply is not wrong for
/// saying more than we asked.
#[derive(serde::Deserialize)]
struct LlmResponse {
    #[serde(default)]
    annotations: Vec<LlmAnnotation>,
}

/// Strip a fenced code block, if the reply is wrapped in one.
///
/// Models fence their JSON far more often than not — 563 of the replies in the
/// first recorded sweep did — so this is the common path, not a fallback.
fn unfence(raw: &str) -> &str {
    let text = raw.trim();
    let Some(stripped) = text.strip_prefix("```") else {
        return text;
    };
    // Drop the opening fence's language tag line, then the closing fence.
    let after_lang = stripped
        .find('\n')
        .map(|n| &stripped[n + 1..])
        .unwrap_or("");
    after_lang.strip_suffix("```").unwrap_or(after_lang).trim()
}

/// Parse the annotations out of a `match-amendments` reply.
pub fn parse_annotations(raw: &str) -> Result<Vec<LlmAnnotation>, String> {
    let response: LlmResponse = serde_json::from_str(unfence(raw)).map_err(|e| e.to_string())?;
    Ok(response.annotations)
}

/// The word-level change shape the model emits for `extract-changes`.
#[derive(serde::Deserialize)]
struct RawDiff {
    #[serde(default)]
    added: Vec<String>,
    #[serde(default)]
    removed: Vec<String>,
}

/// Pull the JSON array out of `<response>...</response>` and into `BillDiff`s.
pub fn parse_changes(raw: &str) -> Result<Vec<crate::legislature::BillDiff>, String> {
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
        .map(|d| crate::legislature::BillDiff {
            added: d.added,
            removed: d.removed,
        })
        .collect())
}
