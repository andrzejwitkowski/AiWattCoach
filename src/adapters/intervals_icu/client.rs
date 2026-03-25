use futures::future::try_join_all;
use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt as _};
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::{multipart, Client, RequestBuilder, StatusCode};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::domain::intervals::{
    Activity, ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
    ActivityStream, ActivityZoneTime, BoxFuture, CreateEvent, DateRange, Event, EventCategory,
    IntervalsApiPort, IntervalsConnectionError, IntervalsConnectionTester, IntervalsCredentials,
    IntervalsError, UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};

use super::dto::{
    ActivityIntervalGroupResponse, ActivityIntervalResponse, ActivityResponse,
    ActivityStreamResponse, CreateEventRequest, EventResponse, UpdateActivityRequest,
    UpdateEventRequest, UploadResponse,
};

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

    pub fn with_timeouts(
        connect_timeout_secs: u64,
        timeout_secs: u64,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(connect_timeout_secs))
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?;
        Ok(Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    fn athlete_url(&self, athlete_id: &str, path: &str) -> String {
        Self::athlete_url_impl(&self.base_url, athlete_id, path)
    }

    fn athlete_url_impl(base_url: &str, athlete_id: &str, path: &str) -> String {
        format!("{base_url}/api/v1/athlete/{athlete_id}{path}")
    }

    fn with_trace_context(request: RequestBuilder) -> RequestBuilder {
        let context = tracing::Span::current().context();

        if !context.span().span_context().is_valid() {
            return request;
        }

        let mut headers = reqwest::header::HeaderMap::new();
        TraceContextPropagator::new().inject_context(&context, &mut HeaderInjector(&mut headers));

        request.headers(headers)
    }

    fn activity_url(&self, activity_id: &str, path: &str) -> String {
        Self::activity_url_impl(&self.base_url, activity_id, path)
    }

    fn activity_url_impl(base_url: &str, activity_id: &str, path: &str) -> String {
        format!("{base_url}/api/v1/activity/{activity_id}{path}")
    }

    async fn fetch_activity_details(
        client: Client,
        base_url: String,
        credentials: IntervalsCredentials,
        activity_id: String,
    ) -> Result<Activity, IntervalsError> {
        let url = Self::activity_url_impl(&base_url, &activity_id, "");
        let streams_url = Self::activity_url_impl(&base_url, &activity_id, "/streams");

        let response = Self::with_trace_context(
            client
                .get(url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .query(&[("intervals", "true")]),
        )
        .send()
        .await
        .map_err(map_connection_error)?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(IntervalsError::NotFound);
        }

        let response = response.error_for_status().map_err(map_api_error)?;
        let payload: ActivityResponse = response.json().await.map_err(map_api_error)?;
        let mut activity = map_activity_response(payload);

        if activity.stream_types.is_empty() {
            return Ok(activity);
        }

        let streams_response = Self::with_trace_context(
            client
                .get(streams_url)
                .basic_auth("API_KEY", Some(&credentials.api_key))
                .query(&[("types", activity.stream_types.join(","))]),
        )
        .send()
        .await
        .map_err(map_connection_error)?;

        let streams_response = streams_response.error_for_status().map_err(map_api_error)?;
        let streams: Vec<ActivityStreamResponse> =
            streams_response.json().await.map_err(map_api_error)?;
        activity.details.streams = streams.into_iter().map(map_activity_stream).collect();

        Ok(activity)
    }
}

impl IntervalsConnectionTester for IntervalsIcuClient {
    fn test_connection(
        &self,
        api_key: &str,
        athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let api_key = api_key.to_string();
        let athlete_id = athlete_id.to_string();

        Box::pin(async move {
            let url = Self::athlete_url_impl(&base_url, &athlete_id, "");

            let response = client.get(&url).basic_auth("API_KEY", Some(&api_key));
            let response = Self::with_trace_context(response)
                .send()
                .await
                .map_err(|_| IntervalsConnectionError::Unavailable)?;

            let status = response.status();

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(IntervalsConnectionError::Unauthenticated);
            }

            if status == StatusCode::NOT_FOUND {
                return Err(IntervalsConnectionError::InvalidConfiguration);
            }

            if !status.is_success() {
                return Err(IntervalsConnectionError::Unavailable);
            }

            Ok(())
        })
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
                .query(&[("oldest", &range.oldest), ("newest", &range.newest)]);
            let response = Self::with_trace_context(response)
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
            let payload: Vec<ActivityResponse> = response.json().await.map_err(map_api_error)?;
            Ok(payload.into_iter().map(map_activity_response).collect())
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

fn map_connection_error(error: reqwest::Error) -> IntervalsError {
    IntervalsError::ConnectionError(error.to_string())
}

fn map_api_error(error: reqwest::Error) -> IntervalsError {
    let message = error.to_string();

    match error.status() {
        Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => {
            IntervalsError::CredentialsNotConfigured
        }
        _ => IntervalsError::ApiError(message),
    }
}

fn map_event_response(response: EventResponse) -> Event {
    Event {
        id: response.id,
        start_date_local: response.start_date_local,
        name: response.name,
        category: EventCategory::from_api_str(&response.category),
        description: response.description,
        indoor: response.indoor.unwrap_or(false),
        color: response.color,
        workout_doc: response.workout_doc,
    }
}

fn map_activity_response(response: ActivityResponse) -> Activity {
    Activity {
        id: response.id,
        athlete_id: response.icu_athlete_id,
        start_date_local: response.start_date_local,
        start_date: response.start_date,
        name: response.name,
        description: response.description,
        activity_type: response.activity_type,
        source: response.source,
        external_id: response.external_id,
        device_name: response.device_name,
        distance_meters: response.distance,
        moving_time_seconds: response.moving_time,
        elapsed_time_seconds: response.elapsed_time,
        total_elevation_gain_meters: response.total_elevation_gain,
        total_elevation_loss_meters: response.total_elevation_loss,
        average_speed_mps: response.average_speed,
        max_speed_mps: response.max_speed,
        average_heart_rate_bpm: response.average_heartrate,
        max_heart_rate_bpm: response.max_heartrate,
        average_cadence_rpm: response.average_cadence,
        trainer: response.trainer.unwrap_or(false),
        commute: response.commute.unwrap_or(false),
        race: response.race.unwrap_or(false),
        has_heart_rate: response.has_heartrate.unwrap_or(false),
        stream_types: response.stream_types.unwrap_or_default(),
        tags: response.tags.unwrap_or_default(),
        metrics: ActivityMetrics {
            training_stress_score: response.icu_training_load,
            normalized_power_watts: response.icu_weighted_avg_watts,
            intensity_factor: response.icu_intensity,
            efficiency_factor: response.icu_efficiency_factor,
            variability_index: response.icu_variability_index,
            average_power_watts: response.icu_average_watts,
            ftp_watts: response.icu_ftp,
            total_work_joules: response.icu_joules,
            calories: response.calories,
            trimp: response.trimp,
            power_load: response.power_load,
            heart_rate_load: response.hr_load,
            pace_load: response.pace_load,
            strain_score: response.strain_score,
        },
        details: ActivityDetails {
            intervals: response
                .icu_intervals
                .unwrap_or_default()
                .into_iter()
                .map(map_activity_interval)
                .collect(),
            interval_groups: response
                .icu_groups
                .unwrap_or_default()
                .into_iter()
                .map(map_activity_interval_group)
                .collect(),
            streams: Vec::new(),
            interval_summary: response.interval_summary.unwrap_or_default(),
            skyline_chart: response.skyline_chart_bytes.unwrap_or_default(),
            power_zone_times: response
                .icu_zone_times
                .unwrap_or_default()
                .into_iter()
                .map(|zone| ActivityZoneTime {
                    zone_id: zone.id,
                    seconds: zone.secs,
                })
                .collect(),
            heart_rate_zone_times: response.icu_hr_zone_times.unwrap_or_default(),
            pace_zone_times: response.pace_zone_times.unwrap_or_default(),
            gap_zone_times: response.gap_zone_times.unwrap_or_default(),
        },
    }
}

fn map_activity_interval(response: ActivityIntervalResponse) -> ActivityInterval {
    ActivityInterval {
        id: response.id,
        label: response.label,
        interval_type: response.interval_type,
        group_id: response.group_id,
        start_index: response.start_index,
        end_index: response.end_index,
        start_time_seconds: response.start_time,
        end_time_seconds: response.end_time,
        moving_time_seconds: response.moving_time,
        elapsed_time_seconds: response.elapsed_time,
        distance_meters: response.distance,
        average_power_watts: response.average_watts,
        normalized_power_watts: response.weighted_average_watts,
        training_stress_score: response.training_load,
        average_heart_rate_bpm: response.average_heartrate,
        average_cadence_rpm: response.average_cadence,
        average_speed_mps: response.average_speed,
        average_stride_meters: response.average_stride,
        zone: response.zone,
    }
}

fn map_activity_interval_group(response: ActivityIntervalGroupResponse) -> ActivityIntervalGroup {
    ActivityIntervalGroup {
        id: response.id,
        count: response.count,
        start_index: response.start_index,
        moving_time_seconds: response.moving_time,
        elapsed_time_seconds: response.elapsed_time,
        distance_meters: response.distance,
        average_power_watts: response.average_watts,
        normalized_power_watts: response.weighted_average_watts,
        training_stress_score: response.training_load,
        average_heart_rate_bpm: response.average_heartrate,
        average_cadence_rpm: response.average_cadence,
        average_speed_mps: response.average_speed,
        average_stride_meters: response.average_stride,
    }
}

fn map_activity_stream(response: ActivityStreamResponse) -> ActivityStream {
    ActivityStream {
        stream_type: response.stream_type,
        name: response.name,
        data: response.data,
        data2: response.data2,
        value_type_is_array: response.value_type_is_array,
        custom: response.custom,
        all_null: response.all_null,
    }
}

fn map_category_to_string(category: &EventCategory) -> String {
    category.as_str().to_string()
}
