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
            Self::Internal(_) => true,
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
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for LlmError {}
