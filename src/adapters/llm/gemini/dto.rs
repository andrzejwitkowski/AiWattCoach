use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct GeminiGenerateContentRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(rename = "cachedContent", skip_serializing_if = "Option::is_none")]
    pub cached_content: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiTextPart>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GeminiTextPart {
    pub text: String,
}

#[derive(Deserialize)]
pub struct GeminiGenerateContentResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata", alias = "usage_metadata")]
    pub usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Deserialize)]
pub struct GeminiCandidate {
    pub content: Option<GeminiContent>,
}

#[derive(Deserialize)]
pub struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount", alias = "prompt_token_count")]
    pub prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount", alias = "candidates_token_count")]
    pub candidates_token_count: Option<u32>,
    #[serde(rename = "totalTokenCount", alias = "total_token_count")]
    pub total_token_count: Option<u32>,
    #[serde(
        rename = "cachedContentTokenCount",
        alias = "cached_content_token_count"
    )]
    pub cached_content_token_count: Option<u32>,
}

#[derive(Serialize)]
pub struct GeminiCreateCacheRequest {
    pub model: String,
    pub contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

#[derive(Deserialize)]
pub struct GeminiCachedContentResponse {
    pub name: Option<String>,
    #[serde(rename = "expireTime")]
    pub expire_time: Option<String>,
}
