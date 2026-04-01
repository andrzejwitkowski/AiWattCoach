use crate::domain::llm::{hash_text, LlmChatRequest};

pub fn context_hash(request: &LlmChatRequest) -> String {
    hash_text(&format!(
        "{}\n{}",
        request.system_prompt, request.stable_context
    ))
}
