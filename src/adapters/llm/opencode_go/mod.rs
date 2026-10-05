pub mod client;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Responses,
    ChatCompletions,
    Messages,
}

pub fn protocol_for_model(model: &str) -> Option<Protocol> {
    match model {
        "gpt-6-luna"
        | "gpt-5.6-luna"
        | "grok-4.6"
        | "grok-4.7"
        | "muse-spark-1.2-contributor"
        | "muse-spark-1.3-contributor" => Some(Protocol::Responses),
        "minimax-m3" | "minimax-m2.7" | "qwen3.8-max" | "qwen3.8-flash" | "qwen3.7-plus" => {
            Some(Protocol::Messages)
        }
        "glm-5.3-flash"
        | "glm-5.3"
        | "glm-5.2"
        | "kimi-k3"
        | "kimi-k2.7-code"
        | "kimi-k2.6"
        | "longcat-2.0"
        | "longcat-2.5-preview-free"
        | "deepseek-v4.1-flash"
        | "deepseek-v4-pro"
        | "deepseek-v4-flash"
        | "deepseek-v4-flash-vision-exp"
        | "mimo-v2.6-flash"
        | "mimo-v2.6-pro"
        | "mimo-v2.5"
        | "mimo-v2.5-pro"
        | "hy4-preview"
        | "hy3"
        | "space-bunny-free" => Some(Protocol::ChatCompletions),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{protocol_for_model, Protocol};

    #[test]
    fn catalog_routes_luna_to_responses_and_other_go_models() {
        assert_eq!(
            protocol_for_model("gpt-5.6-luna"),
            Some(Protocol::Responses)
        );
        assert_eq!(
            protocol_for_model("glm-5.3"),
            Some(Protocol::ChatCompletions)
        );
        assert_eq!(protocol_for_model("qwen3.8-max"), Some(Protocol::Messages));
        assert_eq!(protocol_for_model("unknown"), None);
    }
}
