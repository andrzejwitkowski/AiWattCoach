use crate::domain::llm::{hash_text, LlmChatRequest};

pub fn context_hash(request: &LlmChatRequest) -> String {
    hash_text(&format!(
        "system:{}:{}context:{}:{}",
        request.system_prompt.len(),
        request.system_prompt,
        request.stable_context.len(),
        request.stable_context
    ))
}
