#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmError {
    CredentialsNotConfigured,
    ProviderNotConfigured,
    ModelNotConfigured,
    UnsupportedProvider(String),
    Transport(String),
    ProviderRejected(String),
    RateLimited(String),
    InvalidResponse(String),
    Internal(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CredentialsNotConfigured => write!(f, "LLM credentials are not configured"),
            Self::ProviderNotConfigured => write!(f, "LLM provider is not configured"),
            Self::ModelNotConfigured => write!(f, "LLM model is not configured"),
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
