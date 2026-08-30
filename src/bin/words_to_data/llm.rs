//! Minimal synchronous client for OpenAI-compatible chat-completions APIs.
//!
//! Targets a local `llama.cpp` server by default but works against any
//! `/v1/chat/completions` endpoint (DeepSeek, etc.). Deliberately blocking:
//! callers that want concurrency spawn OS threads and share `&LlmClient`,
//! which is `Send + Sync` because `ureq::Agent` is.

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
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: None,
        }
    }
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

        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut last_err = String::new();
        for attempt in 0..self.max_retries {
            match self.try_once(&url, &body) {
                Ok(content) => return Ok(content),
                Err(err) => {
                    last_err = err;
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

    /// A single request attempt (no retry).
    fn try_once(&self, url: &str, body: &serde_json::Value) -> Result<String, String> {
        let mut request = self
            .agent
            .post(url)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", &format!("Bearer {key}"));
        }

        // Serialize/parse JSON ourselves so we don't need ureq's optional `json` feature.
        let payload = serde_json::to_vec(body).map_err(|e| e.to_string())?;
        let text = request
            .send(&payload)
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

        value["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("Unexpected response shape: {value}"))
    }
}
