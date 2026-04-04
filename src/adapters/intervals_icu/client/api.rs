use futures::future::try_join_all;
use reqwest::{multipart, StatusCode};
use serde_json::Value;

use crate::adapters::intervals_icu::dto::{
    ActivityResponse, CreateEventRequest, EventResponse, UpdateActivityRequest, UpdateEventRequest,
    UploadResponse,
};
use crate::domain::intervals::{
    Activity, BoxFuture, CreateEvent, DateRange, Event, IntervalsApiPort, IntervalsCredentials,
    IntervalsError, UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};

use super::{
    errors::{map_api_error, map_connection_error, map_error_response},
    mapping::{map_activity_response, map_category_to_string, map_event_response},
    truncate_logged_response_body, IntervalsIcuClient,
};

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
            tracing::info!(
                provider = "intervals_icu",
                method = "GET",
                url = %url,
                oldest = %range.oldest,
                newest = %range.newest,
                "sending intervals events request"
            );

            let response = client
                .get(url.clone())
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .query(&[("oldest", &range.oldest), ("newest", &range.newest)]);
            let response = Self::with_trace_context(response)
                .send()
                .await
                .map_err(|error| {
                    let error = map_connection_error(error);
                    tracing::warn!(
                        provider = "intervals_icu",
                        method = "GET",
                        url = %url,
                        error = %error,
                        "intervals events transport failure"
                    );
                    error
                })?;

            if !response.status().is_success() {
                let failure = map_error_response(response).await;
                tracing::warn!(
                    provider = "intervals_icu",
                    method = "GET",
                    url = %url,
                    status = failure.status.map(|status| status.as_u16()).unwrap_or_default(),
                    error = %failure.error,
                    response_body = %failure
                        .response_body
                        .as_deref()
                        .map(truncate_logged_response_body)
                        .unwrap_or_default(),
                    "intervals events request failed"
                );
                return Err(failure.error);
            }

            let response_body = response.text().await.map_err(|error| {
                let message = error.without_url().to_string();
                tracing::warn!(
                    provider = "intervals_icu",
                    method = "GET",
                    url = %url,
                    error = %message,
                    "intervals events response body read failed"
                );
                IntervalsError::ApiError(message)
            })?;

            let payload: Vec<Value> = serde_json::from_str(&response_body).map_err(|error| {
                let message = error.to_string();
                tracing::warn!(
                    provider = "intervals_icu",
                    method = "GET",
                    url = %url,
                    error = %message,
                    response_body = %truncate_logged_response_body(&response_body),
                    "intervals events response json parsing failed"
                );
                IntervalsError::ApiError(message)
            })?;

            let mut events = Vec::with_capacity(payload.len());
            for (index, value) in payload.into_iter().enumerate() {
                let event_id = value.get("id").map(|id| match id {
                    Value::String(value) => value.clone(),
                    Value::Number(value) => value.to_string(),
                    other => other.to_string(),
                });

                match serde_json::from_value::<EventResponse>(value.clone()) {
                    Ok(event) => events.push(map_event_response(event)),
                    Err(error) => {
                        tracing::warn!(
                            provider = "intervals_icu",
                            method = "GET",
                            url = %url,
                            event_index = index,
                            event_id = event_id.as_deref().unwrap_or(""),
                            error = %error,
                            event_body = %truncate_logged_response_body(&value.to_string()),
                            "skipping malformed intervals event from list response"
                        );
                    }
                }
            }

            tracing::info!(
                provider = "intervals_icu",
                method = "GET",
                url = %url,
                event_count = events.len(),
                "intervals events request succeeded"
            );

            Ok(events)
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
                .basic_auth("API_KEY", Some(&credentials.api_key));
            let response = Self::with_trace_context(response)
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
            let file_upload = event.file_upload;
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
                    file_contents: file_upload
                        .as_ref()
                        .and_then(|file| file.file_contents.clone()),
                    file_contents_base64: file_upload
                        .as_ref()
                        .and_then(|file| file.file_contents_base64.clone()),
                    filename: file_upload.as_ref().map(|file| file.filename.clone()),
                });
            let response = Self::with_trace_context(response)
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
            let file_upload = event.file_upload;
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
                    file_contents: file_upload
                        .as_ref()
                        .and_then(|file| file.file_contents.clone()),
                    file_contents_base64: file_upload
                        .as_ref()
                        .and_then(|file| file.file_contents_base64.clone()),
                    filename: file_upload.as_ref().map(|file| file.filename.clone()),
                });
            let response = Self::with_trace_context(response)
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
                .basic_auth("API_KEY", Some(&credentials.api_key));
            let response = Self::with_trace_context(response)
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
                .basic_auth("API_KEY", Some(&credentials.api_key));
            let response = Self::with_trace_context(response)
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

    fn list_activities(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let range = range.clone();
        let url = self.athlete_url(&credentials.athlete_id, "/activities");

        Box::pin(async move {
            let response = client
                .get(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .query(&[("oldest", &range.oldest), ("newest", &range.newest)])
                .send()
                .await
                .map_err(map_connection_error)?;

            let response = response.error_for_status().map_err(map_api_error)?;
            let payload: Vec<Value> = response.json().await.map_err(map_api_error)?;

            Ok(payload
                .into_iter()
                .filter_map(|value| {
                    let activity_id = value
                        .get("id")
                        .and_then(|raw| raw.as_str().map(str::to_owned));

                    match serde_json::from_value::<ActivityResponse>(value) {
                        Ok(activity) => Some(map_activity_response(activity)),
                        Err(error) => {
                            tracing::warn!(
                                activity_id,
                                %error,
                                "skipping malformed intervals activity from list response"
                            );
                            None
                        }
                    }
                })
                .collect())
        })
    }

    fn get_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let credentials = credentials.clone();
        let activity_id = activity_id.to_string();

        Box::pin(async move {
            Self::fetch_activity_details(client, base_url, credentials, activity_id).await
        })
    }

    fn upload_activity(
        &self,
        credentials: &IntervalsCredentials,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let credentials = credentials.clone();
        let url = self.athlete_url(&credentials.athlete_id, "/activities");

        Box::pin(async move {
            let mut request = client
                .post(url)
                .basic_auth("API_KEY", Some(&credentials.api_key));

            let mut query_params: Vec<(&str, String)> = Vec::new();
            if let Some(name) = upload.name.as_ref() {
                query_params.push(("name", name.clone()));
            }
            if let Some(description) = upload.description.as_ref() {
                query_params.push(("description", description.clone()));
            }
            if let Some(device_name) = upload.device_name.as_ref() {
                query_params.push(("device_name", device_name.clone()));
            }
            if let Some(external_id) = upload.external_id.as_ref() {
                query_params.push(("external_id", external_id.clone()));
            }
            if let Some(paired_event_id) = upload.paired_event_id {
                query_params.push(("paired_event_id", paired_event_id.to_string()));
            }
            if !query_params.is_empty() {
                request = request.query(&query_params);
            }

            let form = multipart::Form::new().part(
                "file",
                multipart::Part::bytes(upload.file_bytes).file_name(upload.filename),
            );

            let response = request
                .multipart(form)
                .send()
                .await
                .map_err(map_connection_error)?;
            let created = response.status() == StatusCode::CREATED;
            let response = response.error_for_status().map_err(map_api_error)?;
            let payload: UploadResponse = response.json().await.map_err(map_api_error)?;
            let activity_ids: Vec<String> = payload
                .activities
                .unwrap_or_default()
                .into_iter()
                .map(|activity| activity.id)
                .collect();

            let activities = try_join_all(activity_ids.iter().cloned().map(|activity_id| {
                Self::fetch_activity_details(
                    client.clone(),
                    base_url.clone(),
                    credentials.clone(),
                    activity_id,
                )
            }))
            .await?;

            Ok(UploadedActivities {
                created,
                activity_ids,
                activities,
            })
        })
    }

    fn update_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let credentials = credentials.clone();
        let activity_id = activity_id.to_string();
        let url = self.activity_url(&activity_id, "");

        Box::pin(async move {
            let response = client
                .put(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .json(&UpdateActivityRequest {
                    name: activity.name,
                    description: activity.description,
                    activity_type: activity.activity_type,
                    trainer: activity.trainer,
                    commute: activity.commute,
                    race: activity.race,
                })
                .send()
                .await
                .map_err(map_connection_error)?;

            if response.status() == StatusCode::NOT_FOUND {
                return Err(IntervalsError::NotFound);
            }

            let response = response.error_for_status().map_err(map_api_error)?;
            let _: ActivityResponse = response.json().await.map_err(map_api_error)?;
            Self::fetch_activity_details(client, base_url, credentials, activity_id).await
        })
    }

    fn delete_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let client = self.client.clone();
        let credentials = credentials.clone();
        let activity_id = activity_id.to_string();
        let url = self.activity_url(&activity_id, "");

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
}
