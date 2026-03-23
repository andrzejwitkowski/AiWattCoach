use reqwest::{Client, StatusCode};

use crate::domain::intervals::{
    BoxFuture, CreateEvent, DateRange, Event, EventCategory, IntervalsApiPort,
    IntervalsCredentials, IntervalsError, UpdateEvent,
};

use super::dto::{CreateEventRequest, EventResponse, UpdateEventRequest};

const DEFAULT_BASE_URL: &str = "https://intervals.icu";

#[derive(Clone)]
pub struct IntervalsIcuClient {
    client: Client,
    base_url: String,
}

impl IntervalsIcuClient {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    fn athlete_url(&self, athlete_id: &str, path: &str) -> String {
        format!("{}/api/v1/athlete/{}{}", self.base_url, athlete_id, path)
    }
}

impl IntervalsApiPort for IntervalsIcuClient {
    fn list_events(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let range = range.clone();
        let url = self.athlete_url(&credentials.athlete_id, "/events.json");

        Box::pin(async move {
            let response = client
                .get(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .query(&[("oldest", &range.oldest), ("newest", &range.newest)])
                .send()
                .await
                .map_err(map_connection_error)?;

            let response = response.error_for_status().map_err(map_api_error)?;
            let payload: Vec<EventResponse> = response.json().await.map_err(map_api_error)?;

            Ok(payload.into_iter().map(map_event_response).collect())
        })
    }

    fn get_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let url = self.athlete_url(&credentials.athlete_id, &format!("/events/{event_id}"));

        Box::pin(async move {
            let response = client
                .get(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .send()
                .await
                .map_err(map_connection_error)?;

            if response.status() == StatusCode::NOT_FOUND {
                return Err(IntervalsError::NotFound);
            }

            let response = response.error_for_status().map_err(map_api_error)?;
            let payload: EventResponse = response.json().await.map_err(map_api_error)?;

            Ok(map_event_response(payload))
        })
    }

    fn create_event(
        &self,
        credentials: &IntervalsCredentials,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let url = self.athlete_url(&credentials.athlete_id, "/events");

        Box::pin(async move {
            let response = client
                .post(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .json(&CreateEventRequest {
                    category: map_category_to_string(&event.category),
                    start_date_local: event.start_date_local,
                    name: event.name,
                    description: event.description,
                    indoor: event.indoor,
                    color: event.color,
                    workout_doc: event.workout_doc,
                })
                .send()
                .await
                .map_err(map_connection_error)?;

            let response = response.error_for_status().map_err(map_api_error)?;
            let payload: EventResponse = response.json().await.map_err(map_api_error)?;

            Ok(map_event_response(payload))
        })
    }

    fn update_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let url = self.athlete_url(&credentials.athlete_id, &format!("/events/{event_id}"));

        Box::pin(async move {
            let response = client
                .put(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .json(&UpdateEventRequest {
                    category: event.category.as_ref().map(map_category_to_string),
                    start_date_local: event.start_date_local,
                    name: event.name,
                    description: event.description,
                    indoor: event.indoor,
                    color: event.color,
                    workout_doc: event.workout_doc,
                })
                .send()
                .await
                .map_err(map_connection_error)?;

            if response.status() == StatusCode::NOT_FOUND {
                return Err(IntervalsError::NotFound);
            }

            let response = response.error_for_status().map_err(map_api_error)?;
            let payload: EventResponse = response.json().await.map_err(map_api_error)?;

            Ok(map_event_response(payload))
        })
    }

    fn delete_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let url = self.athlete_url(&credentials.athlete_id, &format!("/events/{event_id}"));

        Box::pin(async move {
            let response = client
                .delete(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .send()
                .await
                .map_err(map_connection_error)?;

            if response.status() == StatusCode::NOT_FOUND {
                return Err(IntervalsError::NotFound);
            }

            response.error_for_status().map_err(map_api_error)?;
            Ok(())
        })
    }

    fn download_fit(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let url = self.athlete_url(
            &credentials.athlete_id,
            &format!("/events/{event_id}/download.fit"),
        );

        Box::pin(async move {
            let response = client
                .get(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .send()
                .await
                .map_err(map_connection_error)?;

            if response.status() == StatusCode::NOT_FOUND {
                return Err(IntervalsError::NotFound);
            }

            let response = response.error_for_status().map_err(map_api_error)?;
            let bytes = response.bytes().await.map_err(map_connection_error)?;
            Ok(bytes.to_vec())
        })
    }
}

fn map_connection_error(error: reqwest::Error) -> IntervalsError {
    let _ = error;
    IntervalsError::ConnectionError("Failed to reach Intervals.icu".to_string())
}

fn map_api_error(error: reqwest::Error) -> IntervalsError {
    let _ = error;
    IntervalsError::ApiError("Intervals.icu request failed".to_string())
}

fn map_event_response(response: EventResponse) -> Event {
    Event {
        id: response.id,
        start_date_local: response.start_date_local,
        name: response.name,
        category: parse_category(&response.category),
        description: response.description,
        indoor: response.indoor.unwrap_or(false),
        color: response.color,
        workout_doc: response.workout_doc,
    }
}

fn parse_category(category: &str) -> EventCategory {
    match category {
        "WORKOUT" => EventCategory::Workout,
        "RACE" => EventCategory::Race,
        "NOTE" => EventCategory::Note,
        "TARGET" => EventCategory::Target,
        "SEASON" => EventCategory::Season,
        _ => EventCategory::Other,
    }
}

fn map_category_to_string(category: &EventCategory) -> String {
    match category {
        EventCategory::Workout => "WORKOUT".to_string(),
        EventCategory::Race => "RACE".to_string(),
        EventCategory::Note => "NOTE".to_string(),
        EventCategory::Target => "TARGET".to_string(),
        EventCategory::Season => "SEASON".to_string(),
        EventCategory::Other => "OTHER".to_string(),
    }
}
