use crate::{BackendKind, ChatMessage};

/// A compression strategy for fitting conversation history into a model's
/// context window.
///
/// Implementations should preserve as much useful context as possible while
/// respecting the token budget.
pub trait CompressionStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn compress(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        backend: BackendKind,
        context_window: usize,
        max_output_tokens: usize,
    ) -> Vec<ChatMessage>;
}
