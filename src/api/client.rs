//! HTTP client for Chat Completions + SSE.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::sse::SseParser;
use super::types::*;

const APP_TITLE: &str = "Muninn";
const APP_REFERER: &str = "https://github.com/d3adp0rt/muninn";
const MAX_ERROR_BODY: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Usually `https://openrouter.ai/api/v1`.
    pub base_url: String,
    pub api_key: String,
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

pub type ChatStream = ReceiverStream<Result<StreamEvent, ApiError>>;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Response metadata, sent once when the first chunk arrives.
    Meta {
        id: Option<String>,
        model: Option<String>,
        provider: Option<String>,
    },
    Content(String),
    Reasoning(String),
    ReasoningDetails(serde_json::Value),
    ToolCalls(Vec<ToolCallDelta>),
    Annotations(Vec<serde_json::Value>),
    Finish(String),
    Usage(Usage),
    Done,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("invalid base URL: {0}")]
    Url(String),

    #[error("HTTP {status}: {message}")]
    Status { status: u16, message: String },

    #[error("could not decode response: {0}")]
    Decode(String),

    #[error("provider error: {message}")]
    Remote {
        message: String,
        code: Option<String>,
    },

    #[error("stream ended unexpectedly")]
    Incomplete,

    #[error("cancelled")]
    Cancelled,
}

impl Client {
    pub fn new(config: ClientConfig) -> Result<Self, ApiError> {
        let base = config.base_url.trim().trim_end_matches('/').to_owned();
        if !(base.starts_with("https://") || base.starts_with("http://")) {
            return Err(ApiError::Url(config.base_url));
        }

        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .user_agent(concat!("Muninn/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            http,
            base_url: base,
            api_key: config.api_key,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn headers(&self, stream: bool) -> Result<HeaderMap, ApiError> {
        let mut headers = HeaderMap::new();
        let mut auth = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|_| ApiError::Url("API key contains invalid characters".into()))?;
        auth.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, auth);

        headers.insert("HTTP-Referer", HeaderValue::from_static(APP_REFERER));
        headers.insert("X-Title", HeaderValue::from_static(APP_TITLE));

        if stream {
            headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        } else {
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        }
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    /// Start a streaming chat completion.
    ///
    /// The returned stream ends with `Done` or an error. Dropping it cancels
    /// the HTTP request as soon as the next send fails.
    pub fn stream_chat(&self, request: ChatRequest) -> ChatStream {
        let (tx, rx) = mpsc::channel(64);
        let this = self.clone();

        tokio::spawn(async move {
            if let Err(err) = this.run_stream(request, &tx).await {
                let _ = tx.send(Err(err)).await;
            }
        });

        ReceiverStream::new(rx)
    }

    async fn run_stream(
        &self,
        request: ChatRequest,
        tx: &mpsc::Sender<Result<StreamEvent, ApiError>>,
    ) -> Result<(), ApiError> {
        let url = self.endpoint("chat/completions");
        let mut req = self
            .http
            .post(url)
            .headers(self.headers(true)?)
            .json(&request);

        // SSE responses should not be buffered by intermediaries.
        req = req.header("Cache-Control", "no-cache");

        let response = req.send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = read_limited(response, MAX_ERROR_BODY).await;
            return Err(ApiError::Status {
                status: status.as_u16(),
                message: extract_error_message(&body),
            });
        }

        let mut parser = SseParser::new();
        let mut bytes = response.bytes_stream();
        let mut sent_meta = false;
        let mut saw_done = false;

        while let Some(chunk) = bytes.next().await {
            let chunk = chunk?;
            for event in parser.push(&chunk) {
                if event.data.trim() == "[DONE]" {
                    saw_done = true;
                    let _ = tx.send(Ok(StreamEvent::Done)).await;
                    break;
                }
                let parsed = parse_event(&event.data)?;

                if !sent_meta {
                    sent_meta = true;
                    let _ = tx
                        .send(Ok(StreamEvent::Meta {
                            id: parsed.0.id.clone(),
                            model: parsed.0.model.clone(),
                            provider: parsed.0.provider.clone(),
                        }))
                        .await;
                }

                for ev in parsed.1 {
                    if tx.send(Ok(ev)).await.is_err() {
                        return Err(ApiError::Cancelled);
                    }
                }
            }
            if saw_done {
                break;
            }
        }

        for event in parser.finish() {
            if event.data.trim() == "[DONE]" {
                saw_done = true;
                continue;
            }
            let parsed = parse_event(&event.data)?;
            for ev in parsed.1 {
                let _ = tx.send(Ok(ev)).await;
            }
        }

        if !saw_done {
            let _ = tx.send(Err(ApiError::Incomplete)).await;
        }
        Ok(())
    }

    /// Fetch the model catalog.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, ApiError> {
        let url = self.endpoint("models");
        let response = self
            .http
            .get(url)
            .headers(self.headers(false)?)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = read_limited(response, MAX_ERROR_BODY).await;
            return Err(ApiError::Status {
                status: status.as_u16(),
                message: extract_error_message(&body),
            });
        }

        let body: ModelsResponse = response.json().await?;
        Ok(body.data)
    }

    /// OpenRouter only: key metadata (usage, limits, free-tier flag).
    pub async fn key_info(&self) -> Result<serde_json::Value, ApiError> {
        self.get_json("key").await
    }

    /// OpenRouter only: credit balance.
    pub async fn credits(&self) -> Result<serde_json::Value, ApiError> {
        self.get_json("credits").await
    }

    async fn get_json(&self, path: &str) -> Result<serde_json::Value, ApiError> {
        let response = self
            .http
            .get(self.endpoint(path))
            .headers(self.headers(false)?)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = read_limited(response, MAX_ERROR_BODY).await;
            return Err(ApiError::Status {
                status: status.as_u16(),
                message: extract_error_message(&body),
            });
        }

        Ok(response.json::<serde_json::Value>().await?)
    }
}

fn parse_event(data: &str) -> Result<(StreamChunk, Vec<StreamEvent>), ApiError> {
    let chunk: StreamChunk =
        serde_json::from_str(data).map_err(|e| ApiError::Decode(e.to_string()))?;

    if let Some(error) = chunk.error.as_ref() {
        return Err(ApiError::Remote {
            message: error.message.clone(),
            code: error
                .code
                .as_ref()
                .map(|c| c.to_string().trim_matches('"').to_owned()),
        });
    }

    let mut events = Vec::new();

    for choice in &chunk.choices {
        if let Some(reasoning) = choice.delta.reasoning.as_ref()
            && !reasoning.is_empty()
        {
            events.push(StreamEvent::Reasoning(reasoning.clone()));
        }
        if let Some(content) = choice.delta.content.as_ref()
            && !content.is_empty()
        {
            events.push(StreamEvent::Content(content.clone()));
        }
        if let Some(details) = choice.delta.reasoning_details.as_ref() {
            events.push(StreamEvent::ReasoningDetails(details.clone()));
        }
        if let Some(calls) = choice.delta.tool_calls.as_ref()
            && !calls.is_empty()
        {
            events.push(StreamEvent::ToolCalls(calls.clone()));
        }
        if let Some(annotations) = choice.delta.annotations.as_ref()
            && !annotations.is_empty()
        {
            events.push(StreamEvent::Annotations(annotations.clone()));
        }
        if let Some(reason) = choice.finish_reason.as_ref() {
            events.push(StreamEvent::Finish(reason.clone()));
        }
    }

    if let Some(usage) = chunk.usage.as_ref() {
        events.push(StreamEvent::Usage(usage.clone()));
    }

    Ok((chunk, events))
}

async fn read_limited(response: reqwest::Response, limit: usize) -> String {
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(Ok(chunk)) = stream.next().await {
        let remaining = limit.saturating_sub(out.len());
        if remaining == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn extract_error_message(body: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        error: Option<ErrorBody>,
    }

    if let Ok(envelope) = serde_json::from_str::<Envelope>(body)
        && let Some(error) = envelope.error
        && !error.message.is_empty()
    {
        return error.message;
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        "empty error response".into()
    } else {
        trimmed.chars().take(500).collect()
    }
}
