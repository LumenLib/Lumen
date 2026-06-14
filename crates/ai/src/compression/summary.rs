use super::{CompressionResult, CompressionStrategy, message_budget, token_counter};
use crate::{AiService, BackendKind, ChatMessage, ChatRole};
use anyhow::Result;

/// A summary-based compression strategy that condenses older messages into
/// a single synthetic message via an LLM call.
///
/// When the token budget is exceeded, the oldest messages are sent to the
/// LLM for summarisation. A pre-existing summary from a previous session
/// (stored in `ChatSession::compressed_summary`) is included as prior context
/// so the summary accumulates across sessions.
pub struct SummaryCompression;

impl CompressionStrategy for SummaryCompression {
    fn name(&self) -> &'static str {
        "summary"
    }

    fn compress(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        backend: BackendKind,
        context_window: usize,
        max_output_tokens: usize,
    ) -> Vec<ChatMessage> {
        // Sync path: if no LLM call is available, fall back to sliding window.
        let sw = super::sliding_window::SlidingWindow;
        sw.compress(
            messages,
            system_prompt,
            backend,
            context_window,
            max_output_tokens,
        )
    }
}

/// Select older messages for summarisation and call the LLM.
pub async fn summarize_and_compress(
    messages: &[ChatMessage],
    system_prompt: &str,
    backend: BackendKind,
    context_window: usize,
    max_output_tokens: usize,
    existing_summary: &str,
    ai_service: &AiService,
) -> Result<CompressionResult> {
    if messages.is_empty() {
        return Ok(CompressionResult {
            messages: Vec::new(),
            new_summary: None,
        });
    }

    let budget = message_budget(system_prompt, backend, context_window, max_output_tokens);
    let total = token_counter::count_messages_total(messages, system_prompt, backend);

    // No compression needed.
    if total <= budget + 64 {
        return Ok(CompressionResult {
            messages: messages.to_vec(),
            new_summary: None,
        });
    }

    // Reserve the 4 most recent messages for continuity.
    let reserve_start = messages.len().saturating_sub(4);
    let recent = messages[reserve_start..].to_vec();
    let recent_tokens = token_counter::count_message_tokens(&recent, backend);

    // If the recent messages alone exceed budget, fall back to sliding window
    // (summarisation wouldn't help).
    if recent_tokens >= budget {
        let sw = super::sliding_window::SlidingWindow;
        return Ok(CompressionResult {
            messages: sw.compress(
                messages,
                system_prompt,
                backend,
                context_window,
                max_output_tokens,
            ),
            new_summary: None,
        });
    }

    let older = &messages[..reserve_start];
    if older.is_empty() && existing_summary.is_empty() {
        return Ok(CompressionResult {
            messages: messages.to_vec(),
            new_summary: None,
        });
    }

    // Build the summarisation request.
    let summary = call_summary_llm(older, existing_summary, ai_service).await?;

    let summary_msg = ChatMessage {
        role: ChatRole::Assistant,
        content: format!("[Previous conversation summary]\n{}", summary),
        attachments: Vec::new(),
    };

    let compressed: Vec<ChatMessage> = std::iter::once(summary_msg).chain(recent).collect();

    Ok(CompressionResult {
        messages: compressed,
        new_summary: Some(summary),
    })
}

async fn call_summary_llm(
    older: &[ChatMessage],
    existing_summary: &str,
    ai_service: &AiService,
) -> Result<String> {
    let mut summarization_input = Vec::new();

    if !existing_summary.is_empty() {
        summarization_input.push(ChatMessage::assistant(format!(
            "[Previous summary of this conversation]\n{}",
            existing_summary,
        )));
    }

    if !older.is_empty() {
        for msg in older {
            let role_label = match msg.role {
                ChatRole::User => "User",
                ChatRole::Assistant => "Assistant",
                ChatRole::System => "System",
            };
            summarization_input.push(ChatMessage::user(format!(
                "[{}]: {}",
                role_label, msg.content,
            )));
        }
    }

    let prompt = "Summarize the above conversation concisely. \
        Preserve key decisions, user preferences, important facts, \
        and any conclusions. Write in the same language as the conversation. \
        Keep it under 500 words.";

    let result = ai_service.chat(&summarization_input, Some(prompt)).await?;

    Ok(result)
}
