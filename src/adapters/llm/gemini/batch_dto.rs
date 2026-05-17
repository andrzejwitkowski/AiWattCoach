use serde::Deserialize;

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
