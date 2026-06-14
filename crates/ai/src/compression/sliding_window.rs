use super::{CompressionStrategy, message_budget, token_counter};
use crate::{BackendKind, ChatMessage};

/// A sliding-window strategy that drops the oldest messages when the token
/// budget is exceeded.
///
/// The system prompt is always kept. Messages are retained from the newest
/// backwards until the budget runs out. Older messages beyond the budget
/// are discarded.
///
/// This is a zero-cost, deterministic strategy — no extra LLM calls
/// are needed.
pub struct SlidingWindow;

impl CompressionStrategy for SlidingWindow {
    fn name(&self) -> &'static str {
        "sliding_window"
    }

    fn compress(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        backend: BackendKind,
        context_window: usize,
        max_output_tokens: usize,
    ) -> Vec<ChatMessage> {
        if messages.is_empty() {
            return Vec::new();
        }

        let budget = message_budget(system_prompt, backend, context_window, max_output_tokens);

        // Walk from newest to oldest, counting tokens until budget is exceeded.
        let mut total = 0usize;
        let mut keep_from = messages.len();

        for (i, msg) in messages.iter().enumerate().rev() {
            let count = token_counter::count_text_tokens(&msg.content, backend)
                + token_counter::PER_MESSAGE_OVERHEAD;
            if total + count > budget && i < keep_from - 1 {
                // This message doesn't fit; keep everything newer.
                break;
            }
            total += count;
            keep_from = i;
        }

        messages[keep_from..].to_vec()
    }
}
