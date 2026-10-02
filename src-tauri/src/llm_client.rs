//! Minimal chat-completion client for the providers LeanAI routes to.
//!
//! Provider-neutral: callers pass a [`Endpoint`] and a model name from the
//! catalog, never a hard-coded model. Credentials are read from the keychain
//! by the caller at request time and are never logged or put in errors
//! (ADR 0007).

use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";
/// Longest Retry-After LeanAI waits out before retrying a rate-limited call.
const MAX_RETRY_AFTER_SECS: f64 = 30.0;

/// Where a request goes.
pub enum Endpoint {
    Anthropic {
        api_key: String,
    },
    OpenAi {
        api_key: String,
    },
    /// The managed `llama-server` sidecar, OpenAI-compatible, loopback only.
    Local {
        port: u16,
    },
    /// A user-hosted OpenAI-compatible server (Ollama, vLLM, LM Studio,
    /// llama.cpp) on this or another machine. `url` is the full
    /// chat-completions URL, from [`chat_completions_url`].
    SelfHosted {
        url: String,
        api_key: Option<String>,
    },
}

/// Normalises what users paste as a base URL into the chat-completions URL.
/// Accepts `http://host:11434`, `http://host:8000/v1` or the full
/// `.../v1/chat/completions`. Only http and https are allowed.
pub fn chat_completions_url(base: &str) -> AppResult<String> {
    let trimmed = base.trim().trim_end_matches('/');
    let invalid = || {
        AppError::new(
            "invalid_endpoint",
            "Enter the server address as http://host:port, optionally ending in /v1.",
        )
    };
    let parsed = reqwest::Url::parse(trimmed).map_err(|_| invalid())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(invalid());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.query().is_some() {
        return Err(AppError::new(
            "invalid_endpoint",
            "Don't put credentials or query parameters in the address; use the API key field.",
        ));
    }
    Ok(if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    })
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// `user` or `assistant`.
    pub role: &'static str,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct LlmReply {
    pub text: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    /// True when the provider cut the reply at the output limit.
    pub truncated: bool,
    /// True when a requested JSON schema was rejected by the server and the
    /// reply was produced without it (instructions only).
    pub schema_dropped: bool,
}

fn http(timeout_secs: u64) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::internal(format!("HTTP client could not start: {e}")))
}

/// Sends one chat request. Retries once on rate limits, server errors and
/// connection failures; never on 4xx request errors.
///
/// `schema`, when given, asks the provider to constrain the reply to that
/// JSON schema (ADR 0013). Servers that do not support it (older llama.cpp
/// builds, some self-hosted servers, older Claude models) reject the request;
/// it is then sent once more without the schema, and the reply is marked
/// `schema_dropped` so the run can say so.
pub async fn complete(
    endpoint: &Endpoint,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    max_tokens: usize,
    schema: Option<&Value>,
) -> AppResult<LlmReply> {
    match complete_with(endpoint, model, system, messages, max_tokens, schema).await {
        Err(error) if schema.is_some() && error.code == "provider_request_failed" => {
            let mut reply =
                complete_with(endpoint, model, system, messages, max_tokens, None).await?;
            reply.schema_dropped = true;
            Ok(reply)
        }
        other => other,
    }
}

async fn complete_with(
    endpoint: &Endpoint,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    max_tokens: usize,
    schema: Option<&Value>,
) -> AppResult<LlmReply> {
    let mut last_error = None;
    // Rate-limited providers (for example free API tiers) often clear within
    // seconds; `send` waits out a short Retry-After before returning.
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        match send(endpoint, model, system, messages, max_tokens, schema).await {
            Ok(reply) => return Ok(reply),
            Err(error) if error.retryable => last_error = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::internal("provider request failed")))
}

async fn send(
    endpoint: &Endpoint,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    max_tokens: usize,
    schema: Option<&Value>,
) -> AppResult<LlmReply> {
    // Self-hosted models on consumer hardware can be slow; allow them longer.
    let timeout = match endpoint {
        Endpoint::SelfHosted { .. } | Endpoint::Local { .. } => 900,
        _ => 300,
    };
    let client = http(timeout)?;
    let (provider, request) = match endpoint {
        Endpoint::Anthropic { api_key } => {
            let body = anthropic_body(model, system, messages, max_tokens, schema);
            (
                "Anthropic",
                client
                    .post(ANTHROPIC_URL)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .json(&body),
            )
        }
        Endpoint::OpenAi { api_key } => (
            "OpenAI",
            client
                .post(OPENAI_URL)
                .bearer_auth(api_key)
                .json(&openai_body(model, system, messages, max_tokens, schema)),
        ),
        Endpoint::Local { port } => (
            "local model",
            client
                .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
                .json(&openai_body(model, system, messages, max_tokens, schema)),
        ),
        Endpoint::SelfHosted { url, api_key } => {
            let request = client
                .post(url)
                .json(&openai_body(model, system, messages, max_tokens, schema));
            (
                "self-hosted model",
                match api_key {
                    Some(key) => request.bearer_auth(key),
                    None => request,
                },
            )
        }
    };

    let response = request.send().await.map_err(|e| {
        AppError::new(
            "provider_unreachable",
            format!("Could not reach {provider}: {}", describe(&e)),
        )
        .with_recovery(
            "Check the network and that the model server is running and reachable from this computer.",
        )
        .retryable()
    })?;
    let status = response.status();
    // Honour a short Retry-After on rate limits, so a per-minute token limit
    // does not fail the run. Long waits are not worth blocking on.
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|secs| *secs > 0.0 && *secs <= MAX_RETRY_AFTER_SECS);
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.as_u16() == 429 {
        if let Some(secs) = retry_after {
            tokio::time::sleep(Duration::from_secs_f64(secs)).await;
        }
    }

    if !status.is_success() {
        let detail = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("no details")
            .chars()
            .take(300)
            .collect::<String>();
        let error = match status.as_u16() {
            401 | 403 => AppError::new(
                "provider_auth_failed",
                format!("{provider} rejected the credential ({status})."),
            )
            .with_recovery("Re-enter the API key on the Models page."),
            404 => AppError::new(
                "provider_model_unavailable",
                format!("{provider} does not offer model `{model}`: {detail}"),
            )
            .with_recovery("Update the model catalog, or connect another provider."),
            413 => AppError::new(
                "provider_request_too_large",
                format!(
                    "{provider} rejected the request as too large for this model or plan: {detail}"
                ),
            )
            .with_recovery("Use a model with a larger limit, or a more specific request."),
            429 | 500..=599 => AppError::new(
                "provider_unavailable",
                format!("{provider} is busy or failing ({status}): {detail}"),
            )
            .retryable(),
            _ => AppError::new(
                "provider_request_failed",
                format!("{provider} rejected the request ({status}): {detail}"),
            ),
        };
        return Err(error);
    }

    match endpoint {
        Endpoint::Anthropic { .. } => {
            if body.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
                return Err(AppError::new(
                    "provider_refused",
                    "The model declined this request.",
                ));
            }
            let text = body
                .get("content")
                .and_then(Value::as_array)
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                        .filter_map(|b| b.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            Ok(LlmReply {
                text,
                input_tokens: usage(&body, "/usage/input_tokens")
                    + usage(&body, "/usage/cache_read_input_tokens")
                    + usage(&body, "/usage/cache_creation_input_tokens"),
                output_tokens: usage(&body, "/usage/output_tokens"),
                truncated: body.get("stop_reason").and_then(Value::as_str) == Some("max_tokens"),
                schema_dropped: false,
            })
        }
        Endpoint::OpenAi { .. } | Endpoint::Local { .. } | Endpoint::SelfHosted { .. } => {
            let choice = body.pointer("/choices/0");
            Ok(LlmReply {
                text: strip_reasoning(
                    choice
                        .and_then(|c| c.pointer("/message/content"))
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                ),
                input_tokens: usage(&body, "/usage/prompt_tokens"),
                output_tokens: usage(&body, "/usage/completion_tokens"),
                truncated: choice
                    .and_then(|c| c.get("finish_reason"))
                    .and_then(Value::as_str)
                    == Some("length"),
                schema_dropped: false,
            })
        }
    }
}

/// Anthropic Messages request. A schema goes in `output_config.format`
/// (structured outputs); forced tool use is avoided because newer Claude
/// models reject it.
fn anthropic_body(
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    max_tokens: usize,
    schema: Option<&Value>,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    });
    if let Some(schema) = schema {
        body["output_config"] = json!({ "format": { "type": "json_schema", "schema": schema } });
    }
    body
}

/// OpenAI-compatible request (OpenAI, llama-server, self-hosted servers). A
/// schema goes in `response_format` in the strict `json_schema` form, which
/// llama.cpp also turns into a sampling grammar.
fn openai_body(
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    max_tokens: usize,
    schema: Option<&Value>,
) -> Value {
    let mut all = vec![json!({ "role": "system", "content": system })];
    all.extend(
        messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content })),
    );
    let mut body = json!({ "model": model, "max_tokens": max_tokens, "messages": all });
    if let Some(schema) = schema {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": { "name": "leanai_action", "strict": true, "schema": schema },
        });
    }
    body
}

/// Removes `<think>...</think>` reasoning that open-weight models such as
/// Qwen 3 put in the reply text. It is not part of the answer, and braces in
/// it would confuse action parsing. An unclosed block means the reply was cut
/// off while still reasoning, so nothing usable follows it.
pub fn strip_reasoning(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

fn usage(body: &Value, pointer: &str) -> usize {
    body.pointer(pointer).and_then(Value::as_u64).unwrap_or(0) as usize
}

/// reqwest error strings can include the URL; describe the failure instead.
fn describe(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "the request timed out"
    } else if error.is_connect() {
        "the connection failed"
    } else {
        "the request failed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_self_hosted_urls() {
        assert_eq!(
            chat_completions_url("http://192.168.1.20:11434").unwrap(),
            "http://192.168.1.20:11434/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("http://gpu-box:8000/v1/").unwrap(),
            "http://gpu-box:8000/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://llm.example.com/v1/chat/completions").unwrap(),
            "https://llm.example.com/v1/chat/completions"
        );
        assert!(chat_completions_url("file:///etc/passwd").is_err());
        assert!(chat_completions_url("192.168.1.20:11434").is_err());
        assert!(chat_completions_url("http://user:pass@host:1234").is_err());
    }

    #[test]
    fn schema_is_attached_only_when_requested() {
        let messages = [ChatMessage {
            role: "user",
            content: "hi".into(),
        }];
        let schema = json!({ "type": "object" });

        let openai = openai_body("m", "sys", &messages, 100, Some(&schema));
        assert_eq!(openai["response_format"]["type"], "json_schema");
        assert_eq!(openai["response_format"]["json_schema"]["strict"], true);
        assert_eq!(openai["response_format"]["json_schema"]["schema"], schema);
        assert!(openai_body("m", "sys", &messages, 100, None)
            .get("response_format")
            .is_none());

        let anthropic = anthropic_body("m", "sys", &messages, 100, Some(&schema));
        assert_eq!(anthropic["output_config"]["format"]["type"], "json_schema");
        assert_eq!(anthropic["output_config"]["format"]["schema"], schema);
        assert!(anthropic.get("tool_choice").is_none());
        assert!(anthropic_body("m", "sys", &messages, 100, None)
            .get("output_config")
            .is_none());
    }

    #[test]
    fn strips_reasoning_blocks() {
        assert_eq!(
            strip_reasoning("<think>maybe {x}?</think>\n{\"action\":\"answer\",\"text\":\"hi\"}"),
            "{\"action\":\"answer\",\"text\":\"hi\"}"
        );
        assert_eq!(strip_reasoning("plain answer"), "plain answer");
        assert_eq!(strip_reasoning("<think>still thinking when cut"), "");
    }
}
