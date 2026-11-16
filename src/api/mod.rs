//! OpenRouter / OpenAI-compatible HTTP client.
//!
//! The layer is transport-only: it knows how to build requests, parse SSE and
//! report errors. Conversation state, retries and persistence live in `core`.

pub mod client;
pub mod sse;
pub mod types;

pub use client::{ApiError, ChatStream, Client, ClientConfig, StreamEvent};
pub use sse::{SseEvent, SseParser};
pub use types::*;
