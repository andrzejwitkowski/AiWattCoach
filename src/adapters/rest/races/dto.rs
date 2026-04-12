use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct ListRacesQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct RacePath {
    pub race_id: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct UpsertRaceRequest {
    pub date: String,
    pub name: String,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: i32,
    pub discipline: String,
    pub priority: String,
}

#[derive(Serialize)]
pub(super) struct RaceDto {
    #[serde(rename = "raceId")]
    pub race_id: String,
    pub date: String,
    pub name: String,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: i32,
    pub discipline: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}
