#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmError {
    CredentialsNotConfigured,
    ProviderNotConfigured,
    ModelNotConfigured,
    ContextTooLarge(String),
    UnsupportedProvider(String),
    Transport(String),
    ProviderRejected(String),
    RateLimited(String),
    InvalidResponse(String),
    Checkpoint(String),
    Internal(String),
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::CredentialsNotConfigured => false,
            Self::ProviderNotConfigured => false,
            Self::ModelNotConfigured => false,
            Self::ContextTooLarge(_) => false,
            Self::UnsupportedProvider(_) => false,
            Self::Transport(_) => true,
            Self::ProviderRejected(_) => false,
            Self::RateLimited(_) => true,
            Self::InvalidResponse(_) => true,
            Self::Checkpoint(_) => true,
            Self::Internal(_) => true,
        }
    }

    /// Remote provider returned HTTP 401/403 for a request that already had local credentials.
    pub fn provider_auth_rejected(status: u16, body: &str) -> Self {
        let detail = super::logging::truncate_logged_body(body.trim());
        if detail.is_empty() {
            Self::ProviderRejected(format!(
                "Provider rejected the API key (HTTP {status}). Check the key and base URL."
            ))
        } else {
            Self::ProviderRejected(format!(
                "Provider rejected the API key (HTTP {status}): {detail}"
            ))
        }
    }
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CredentialsNotConfigured => write!(f, "LLM credentials are not configured"),
            Self::ProviderNotConfigured => write!(f, "LLM provider is not configured"),
            Self::ModelNotConfigured => write!(f, "LLM model is not configured"),
            Self::ContextTooLarge(message) => write!(f, "{message}"),
            Self::UnsupportedProvider(provider) => {
                write!(f, "Unsupported LLM provider: {provider}")
            }
            Self::Transport(message) => write!(f, "{message}"),
            Self::ProviderRejected(message) => write!(f, "{message}"),
            Self::RateLimited(message) => write!(f, "{message}"),
            Self::InvalidResponse(message) => write!(f, "{message}"),
            Self::Checkpoint(message) => write!(f, "{message}"),
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for LlmError {}

#[cfg(test)]
mod tests {
    use super::LlmError;

    #[test]
    fn provider_auth_rejected_includes_status_and_body() {
        let error = LlmError::provider_auth_rejected(401, r#"{"error":"invalid api key"}"#);
        let message = error.to_string();
        assert!(message.contains("HTTP 401"), "{message}");
        assert!(message.contains("invalid api key"), "{message}");
    }

    #[test]
    fn provider_auth_rejected_has_fallback_without_body() {
        let error = LlmError::provider_auth_rejected(403, "   ");
        let message = error.to_string();
        assert!(message.contains("HTTP 403"), "{message}");
        assert!(message.contains("Check the key and base URL"), "{message}");
    }
}
