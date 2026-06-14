use crate::BackendKind;

const CHARS_PER_TOKEN_FALLBACK: usize = 4;
pub const PER_MESSAGE_OVERHEAD: usize = 4;

/// Count tokens in a plain text string for the given backend.
pub fn count_text_tokens(text: &str, backend: BackendKind) -> usize {
    let bpe = match encoding(backend) {
        Some(bpe) => bpe,
        None => return text.len() / CHARS_PER_TOKEN_FALLBACK,
    };
    bpe.encode_with_special_tokens(text).len()
}

/// Count tokens for a slice of chat messages, including per-message overhead
/// and attachment text.
pub fn count_message_tokens(messages: &[crate::ChatMessage], backend: BackendKind) -> usize {
    let bpe = match encoding(backend) {
        Some(bpe) => bpe,
        None => {
            return messages
                .iter()
                .map(|m| {
                    let content_tokens = m.content.len() / CHARS_PER_TOKEN_FALLBACK;
                    let attach_tokens: usize = m
                        .attachments
                        .iter()
                        .filter_map(|a| a.extracted_text.as_ref())
                        .map(|t| t.len() / CHARS_PER_TOKEN_FALLBACK)
                        .sum();
                    PER_MESSAGE_OVERHEAD + content_tokens + attach_tokens
                })
                .sum();
        }
    };

    let mut total = 0;
    for msg in messages {
        total += PER_MESSAGE_OVERHEAD;
        total += bpe.encode_with_special_tokens(&msg.content).len();
        for att in &msg.attachments {
            if let Some(ref text) = att.extracted_text {
                total += bpe.encode_with_special_tokens(text).len();
            }
        }
    }
    total
}

/// Count total tokens for messages + system prompt combined.
pub fn count_messages_total(
    messages: &[crate::ChatMessage],
    system_prompt: &str,
    backend: BackendKind,
) -> usize {
    let system_tokens = count_text_tokens(system_prompt, backend);
    let msg_tokens = count_message_tokens(messages, backend);
    system_tokens + msg_tokens + PER_MESSAGE_OVERHEAD
}

fn encoding(backend: BackendKind) -> Option<tiktoken_rs::CoreBPE> {
    match backend {
        BackendKind::OpenAI => tiktoken_rs::o200k_base().ok(),
        BackendKind::Claude => tiktoken_rs::cl100k_base().ok(),
        BackendKind::Ollama => tiktoken_rs::cl100k_base().ok(),
    }
}
