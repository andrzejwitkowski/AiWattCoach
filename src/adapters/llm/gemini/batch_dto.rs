use serde::{Deserialize, Serialize};

use super::dto::{GeminiContent, GeminiGenerationConfig};

#[derive(Serialize)]
pub struct GeminiBatchCreateRequest {
    pub batch: GeminiBatchCreateBody,
}

#[derive(Serialize)]
pub struct GeminiBatchCreateBody {
    #[serde(rename = "display_name")]
    pub display_name: String,
    #[serde(rename = "input_config")]
    pub input_config: GeminiBatchInputConfig,
}

#[derive(Serialize)]
pub struct GeminiBatchInputConfig {
    pub requests: GeminiBatchInlineRequests,
}

#[derive(Serialize)]
pub struct GeminiBatchInlineRequests {
    pub requests: Vec<GeminiBatchInlineRequest>,
}

#[derive(Serialize)]
pub struct GeminiBatchInlineRequest {
    pub request: GeminiBatchGenerateContentRequest,
    pub metadata: GeminiBatchRequestMetadata,
}

#[derive(Serialize)]
pub struct GeminiBatchGenerateContentRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
pub struct GeminiBatchRequestMetadata {
    pub key: String,
}

#[derive(Deserialize)]
pub struct GeminiBatchCreateResponse {
    pub name: String,
}

#[derive(Deserialize)]
pub struct GeminiBatchGetResponse {
    pub name: String,
    pub metadata: Option<GeminiBatchMetadata>,
    pub response: Option<GeminiBatchResponseBody>,
    pub error: Option<GeminiBatchError>,
}

#[derive(Deserialize)]
pub struct GeminiBatchMetadata {
    pub state: Option<String>,
}

#[derive(Deserialize)]
pub struct GeminiBatchResponseBody {
    #[serde(rename = "responsesFile")]
    pub responses_file: Option<String>,
    #[serde(rename = "inlinedResponses")]
    pub inlined_responses: Option<Vec<GeminiBatchResultLine>>,
}

#[derive(Deserialize)]
pub struct GeminiBatchError {
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct GeminiBatchResultLine {
    pub key: Option<String>,
    pub response: Option<GeminiBatchResultResponse>,
    pub error: Option<GeminiBatchResultError>,
}

#[derive(Deserialize)]
pub struct GeminiBatchResultResponse {
    pub candidates: Option<Vec<GeminiBatchCandidate>>,
}

#[derive(Deserialize)]
pub struct GeminiBatchCandidate {
    pub content: Option<GeminiBatchContent>,
}

#[derive(Deserialize)]
pub struct GeminiBatchContent {
    pub parts: Vec<GeminiBatchTextPart>,
}

#[derive(Deserialize)]
pub struct GeminiBatchTextPart {
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct GeminiBatchResultError {
    pub message: Option<String>,
}
