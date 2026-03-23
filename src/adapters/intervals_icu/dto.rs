use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct EventResponse {
    pub id: i64,
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
    pub indoor: Option<bool>,
    pub color: Option<String>,
    pub workout_doc: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateEventRequest {
    pub category: String,
    pub start_date_local: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub indoor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_doc: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateEventRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_local: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indoor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_doc: Option<String>,
}
