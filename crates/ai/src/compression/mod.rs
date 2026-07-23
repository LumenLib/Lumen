mod sliding_window;
mod strategy;
mod summary;
mod token_counter;

use crate::{AiService, ChatMessage};
use anyhow::Result;
pub use strategy::CompressionStrategy;

/// Result of compressing messages.
pub struct CompressionResult {
    /// The compressed messages to send to the LLM.
    pub messages: Vec<ChatMessage>,
    /// A new summary text to persist (None if unchanged).
    pub new_summary: Option<String>,
}

/// Compress messages to fit within the given context window.
///
/// `messages` — the full conversation history.
/// `system_prompt` — the system prompt (counted separately, always kept).
/// `context_window` — maximum total tokens the model supports.
/// `max_output_tokens` — tokens reserved for the model's response.
/// `strategy` — which compression strategy to use.
/// `existing_summary` — a previously persisted summary (empty if none).
/// `ai_service` — the AI service to use for LLM-based summarisation.
pub async fn compress_messages(
    messages: &[ChatMessage],
    system_prompt: &str,
    backend: crate::BackendKind,
    context_window: usize,
    max_output_tokens: usize,
    strategy: &dyn CompressionStrategy,
    existing_summary: &str,
    ai_service: &AiService,
) -> Result<CompressionResult> {
    let total = token_counter::count_messages_total(messages, system_prompt, backend);
    let budget = message_budget(system_prompt, backend, context_window, max_output_tokens);

    if total <= budget + 64 {
        return Ok(CompressionResult {
            messages: messages.to_vec(),
            new_summary: None,
        });
    }

    if strategy.name() == "summary" {
        summary::summarize_and_compress(
            messages,
            system_prompt,
            backend,
            context_window,
            max_output_tokens,
            existing_summary,
            ai_service,
        )
        .await
    } else {
        let msgs = strategy.compress(
            messages,
            system_prompt,
            backend,
            context_window,
            max_output_tokens,
        );
        Ok(CompressionResult {
            messages: msgs,
            new_summary: None,
        })
    }
}

/// Create a compression strategy by name.
///
/// Supported names: `"sliding_window"`, `"summary"`.
/// Falls back to [`SlidingWindow`] for unknown names.
pub fn create_strategy(name: &str) -> Box<dyn CompressionStrategy> {
    match name {
        "summary" => Box::new(summary::SummaryCompression),
        _ => Box::new(sliding_window::SlidingWindow),
    }
}

/// Compute the total token budget available for messages.
pub fn message_budget(
    system_prompt: &str,
    backend: crate::BackendKind,
    context_window: usize,
    max_output_tokens: usize,
) -> usize {
    let system_tokens = token_counter::count_text_tokens(system_prompt, backend);
    let overhead = 4; // system message overhead
    context_window.saturating_sub(system_tokens + overhead + max_output_tokens)
}
