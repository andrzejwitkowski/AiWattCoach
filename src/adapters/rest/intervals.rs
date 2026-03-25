use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use tracing::Level;

use crate::{
    config::AppState,
    domain::intervals::{
        Activity, ActivityStream, CreateEvent, DateRange, Event, EventCategory, EventFileUpload,
        IntervalsError, UpdateActivity, UpdateEvent, UploadActivity,
    },
};

use super::logging::status_class;

pub const MAX_ACTIVITY_UPLOAD_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_ACTIVITY_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize)]
pub struct ListEventsQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub struct EventPath {
    pub event_id: i64,
}

#[derive(Deserialize)]
pub struct ActivityPath {
    pub activity_id: String,
}

#[derive(Serialize)]
pub struct EventDto {
    pub id: i64,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "eventDefinition")]
    pub event_definition: EventDefinitionDto,
    #[serde(rename = "actualWorkout")]
    pub actual_workout: Option<ActualWorkoutDto>,
}

#[derive(Serialize)]
pub struct EventDefinitionDto {
    #[serde(rename = "rawWorkoutDoc")]
    pub raw_workout_doc: Option<String>,
    pub intervals: Vec<IntervalDefinitionDto>,
}

#[derive(Serialize)]
pub struct IntervalDefinitionDto {
    pub definition: String,
}

#[derive(Serialize)]
pub struct ActualWorkoutDto {
    #[serde(rename = "powerValues")]
    pub power_values: Vec<i32>,
    #[serde(rename = "cadenceValues")]
    pub cadence_values: Vec<i32>,
    #[serde(rename = "heartRateValues")]
    pub heart_rate_values: Vec<i32>,
}

#[derive(Deserialize)]
pub struct CreateEventDto {
    pub category: String,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "workoutDoc")]
    pub workout_doc: Option<String>,
    #[serde(rename = "fileUpload")]
    pub file_upload: Option<EventFileUploadDto>,
}

#[derive(Deserialize)]
pub struct UpdateEventDto {
    pub category: Option<String>,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub indoor: Option<bool>,
    pub color: Option<String>,
    #[serde(rename = "workoutDoc")]
    pub workout_doc: Option<String>,
    #[serde(rename = "fileUpload")]
    pub file_upload: Option<EventFileUploadDto>,
}

#[derive(Deserialize)]
pub struct EventFileUploadDto {
    pub filename: String,
    #[serde(rename = "fileContents")]
    pub file_contents: Option<String>,
    #[serde(rename = "fileContentsBase64")]
    pub file_contents_base64: Option<String>,
}

impl TryFrom<EventFileUploadDto> for EventFileUpload {
    type Error = ();

    fn try_from(value: EventFileUploadDto) -> Result<Self, Self::Error> {
        let file_contents = normalize_optional_upload_field(value.file_contents);
        let file_contents_base64 = normalize_optional_upload_field(value.file_contents_base64);

        if file_contents.is_some() == file_contents_base64.is_some() {
            return Err(());
        }

        Ok(EventFileUpload {
            filename: value.filename,
            file_contents,
            file_contents_base64,
        })
    }
}

#[derive(Serialize)]
pub struct ActivityDto {
    pub id: String,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "activityType")]
    pub activity_type: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: Option<f64>,
    #[serde(rename = "movingTimeSeconds")]
    pub moving_time_seconds: Option<i32>,
    #[serde(rename = "elapsedTimeSeconds")]
    pub elapsed_time_seconds: Option<i32>,
    #[serde(rename = "totalElevationGainMeters")]
    pub total_elevation_gain_meters: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    pub trainer: bool,
    pub commute: bool,
    pub race: bool,
    #[serde(rename = "hasHeartRate")]
    pub has_heart_rate: bool,
    #[serde(rename = "streamTypes")]
    pub stream_types: Vec<String>,
    pub tags: Vec<String>,
    pub metrics: ActivityMetricsDto,
    pub details: ActivityDetailsDto,
}

#[derive(Serialize)]
pub struct ActivityMetricsDto {
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "intensityFactor")]
    pub intensity_factor: Option<f64>,
    #[serde(rename = "efficiencyFactor")]
    pub efficiency_factor: Option<f64>,
    #[serde(rename = "variabilityIndex")]
    pub variability_index: Option<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "ftpWatts")]
    pub ftp_watts: Option<i32>,
    #[serde(rename = "totalWorkJoules")]
    pub total_work_joules: Option<i32>,
    pub calories: Option<i32>,
    pub trimp: Option<f64>,
    #[serde(rename = "powerLoad")]
    pub power_load: Option<i32>,
    #[serde(rename = "heartRateLoad")]
    pub heart_rate_load: Option<i32>,
    #[serde(rename = "paceLoad")]
    pub pace_load: Option<i32>,
    #[serde(rename = "strainScore")]
    pub strain_score: Option<f64>,
}

#[derive(Serialize)]
pub struct ActivityDetailsDto {
    pub intervals: Vec<ActivityIntervalDto>,
    #[serde(rename = "intervalGroups")]
    pub interval_groups: Vec<ActivityIntervalGroupDto>,
    pub streams: Vec<ActivityStreamDto>,
    #[serde(rename = "intervalSummary")]
    pub interval_summary: Vec<String>,
    #[serde(rename = "skylineChart")]
    pub skyline_chart: Vec<String>,
    #[serde(rename = "powerZoneTimes")]
    pub power_zone_times: Vec<ActivityZoneTimeDto>,
    #[serde(rename = "heartRateZoneTimes")]
    pub heart_rate_zone_times: Vec<i32>,
    #[serde(rename = "paceZoneTimes")]
    pub pace_zone_times: Vec<i32>,
    #[serde(rename = "gapZoneTimes")]
    pub gap_zone_times: Vec<i32>,
}

#[derive(Serialize)]
pub struct ActivityIntervalDto {
    pub id: Option<i32>,
    pub label: Option<String>,
    #[serde(rename = "intervalType")]
    pub interval_type: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<i32>,
    #[serde(rename = "endIndex")]
    pub end_index: Option<i32>,
    #[serde(rename = "startTimeSeconds")]
    pub start_time_seconds: Option<i32>,
    #[serde(rename = "endTimeSeconds")]
    pub end_time_seconds: Option<i32>,
    #[serde(rename = "movingTimeSeconds")]
    pub moving_time_seconds: Option<i32>,
    #[serde(rename = "elapsedTimeSeconds")]
    pub elapsed_time_seconds: Option<i32>,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: Option<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<f64>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "averageStrideMeters")]
    pub average_stride_meters: Option<f64>,
    pub zone: Option<i32>,
}

#[derive(Serialize)]
pub struct ActivityIntervalGroupDto {
    pub id: String,
    pub count: Option<i32>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<i32>,
    #[serde(rename = "movingTimeSeconds")]
    pub moving_time_seconds: Option<i32>,
    #[serde(rename = "elapsedTimeSeconds")]
    pub elapsed_time_seconds: Option<i32>,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: Option<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<f64>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "averageStrideMeters")]
    pub average_stride_meters: Option<f64>,
}

#[derive(Serialize)]
pub struct ActivityStreamDto {
    #[serde(rename = "streamType")]
    pub stream_type: String,
    pub name: Option<String>,
    pub data: Option<Value>,
    pub data2: Option<Value>,
    #[serde(rename = "valueTypeIsArray")]
    pub value_type_is_array: bool,
    pub custom: bool,
    #[serde(rename = "allNull")]
    pub all_null: bool,
}

#[derive(Serialize)]
pub struct ActivityZoneTimeDto {
    #[serde(rename = "zoneId")]
    pub zone_id: String,
    pub seconds: i32,
}

#[derive(Serialize)]
pub struct UploadActivityResponseDto {
    pub created: bool,
    #[serde(rename = "activityIds")]
    pub activity_ids: Vec<String>,
    pub activities: Vec<ActivityDto>,
}

#[derive(Deserialize)]
pub struct CreateActivityDto {
    pub filename: String,
    #[serde(rename = "fileContentsBase64")]
    pub file_contents_base64: String,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "pairedEventId")]
    pub paired_event_id: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateActivityDto {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "activityType")]
    pub activity_type: Option<String>,
    pub trainer: Option<bool>,
    pub commute: Option<bool>,
    pub race: Option<bool>,
}

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::user_auth::resolve_user_id(state, headers).await
}

pub async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListEventsQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !is_valid_date(&range.oldest) || !is_valid_date(&range.newest) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match intervals_service.list_events(&user_id, &range).await {
        Ok(events) => {
            Json(events.into_iter().map(map_event_to_dto).collect::<Vec<_>>()).into_response()
        }
        Err(error) => map_intervals_error(error),
    }
}

pub async fn get_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service.get_event(&user_id, path.event_id).await {
        Ok(event) => Json(map_event_to_dto(event)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn create_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateEventDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let category = match parse_category(body.category.as_str()) {
        Some(category) => category,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    if !is_valid_date(&body.start_date_local) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let event = CreateEvent {
        category,
        start_date_local: body.start_date_local,
        name: body.name,
        description: body.description,
        indoor: body.indoor,
        color: body.color,
        workout_doc: body.workout_doc,
        file_upload: match try_map_event_file_upload(body.file_upload) {
            Ok(file_upload) => file_upload,
            Err(status) => return status.into_response(),
        },
    };

    match intervals_service.create_event(&user_id, event).await {
        Ok(event) => (StatusCode::CREATED, Json(map_event_to_dto(event))).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn update_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
    Json(body): Json<UpdateEventDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let category = match body.category.as_deref() {
        Some(category) => match parse_category(category) {
            Some(category) => Some(category),
            None => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    if let Some(start_date_local) = body.start_date_local.as_deref() {
        if !is_valid_date(start_date_local) {
            return StatusCode::BAD_REQUEST.into_response();
        }
    }

    let event = UpdateEvent {
        category,
        start_date_local: body.start_date_local,
        name: body.name,
        description: body.description,
        indoor: body.indoor,
        color: body.color,
        workout_doc: body.workout_doc,
        file_upload: match try_map_event_file_upload(body.file_upload) {
            Ok(file_upload) => file_upload,
            Err(status) => return status.into_response(),
        },
    };

    match intervals_service
        .update_event(&user_id, path.event_id, event)
        .await
    {
        Ok(event) => Json(map_event_to_dto(event)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn delete_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service
        .delete_event(&user_id, path.event_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn download_fit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service
        .download_fit(&user_id, path.event_id)
        .await
    {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"event-{}.fit\"", path.event_id),
            )
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn list_activities(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListEventsQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !is_valid_date(&range.oldest) || !is_valid_date(&range.newest) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match intervals_service.list_activities(&user_id, &range).await {
        Ok(activities) => Json(
            activities
                .into_iter()
                .map(map_activity_to_dto)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn get_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ActivityPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service
        .get_activity(&user_id, &path.activity_id)
        .await
    {
        Ok(activity) => Json(map_activity_to_dto(activity)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn create_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateActivityDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let file_bytes = match decode_base64(&body.file_contents_base64) {
        Ok(bytes) => bytes,
        Err(()) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let upload = UploadActivity {
        filename: body.filename,
        file_bytes,
        name: body.name,
        description: body.description,
        device_name: body.device_name,
        external_id: body.external_id,
        paired_event_id: body.paired_event_id,
    };

    match intervals_service.upload_activity(&user_id, upload).await {
        Ok(result) => (
            if result.created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            Json(UploadActivityResponseDto {
                created: result.created,
                activity_ids: result.activity_ids,
                activities: result
                    .activities
                    .into_iter()
                    .map(map_activity_to_dto)
                    .collect(),
            }),
        )
            .into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn update_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ActivityPath>,
    Json(body): Json<UpdateActivityDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let update = UpdateActivity {
        name: body.name,
        description: body.description,
        activity_type: body.activity_type,
        trainer: body.trainer,
        commute: body.commute,
        race: body.race,
    };

    match intervals_service
        .update_activity(&user_id, &path.activity_id, update)
        .await
    {
        Ok(activity) => Json(map_activity_to_dto(activity)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn delete_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ActivityPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service
        .delete_activity(&user_id, &path.activity_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_intervals_error(error),
    }
}

fn map_intervals_error(error: IntervalsError) -> Response {
    match error {
        IntervalsError::Unauthenticated => {
            log_intervals_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        IntervalsError::CredentialsNotConfigured => (
            {
                log_intervals_error(Level::WARN, StatusCode::UNPROCESSABLE_ENTITY, &error);
                StatusCode::UNPROCESSABLE_ENTITY
            },
            "Intervals.icu credentials not configured",
        )
            .into_response(),
        IntervalsError::NotFound => {
            log_intervals_error(Level::WARN, StatusCode::NOT_FOUND, &error);
            StatusCode::NOT_FOUND.into_response()
        }
        IntervalsError::ApiError(_) | IntervalsError::ConnectionError(_) => {
            log_intervals_error(Level::ERROR, StatusCode::BAD_GATEWAY, &error);
            StatusCode::BAD_GATEWAY.into_response()
        }
        IntervalsError::Internal(_) => {
            log_intervals_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn log_intervals_error(level: Level, status: StatusCode, error: &IntervalsError) {
    let error_kind = match error {
        IntervalsError::CredentialsNotConfigured => "credentials_not_configured",
        IntervalsError::NotFound => "not_found",
        IntervalsError::ApiError(_) => "api_error",
        IntervalsError::ConnectionError(_) => "connection_error",
        IntervalsError::Internal(_) => "internal_error",
        IntervalsError::Unauthenticated => "unauthenticated",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "intervals request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "intervals request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

fn map_event_to_dto(event: Event) -> EventDto {
    EventDto {
        id: event.id,
        start_date_local: event.start_date_local,
        name: event.name,
        category: category_to_string(&event.category),
        description: event.description,
        indoor: event.indoor,
        color: event.color,
        event_definition: EventDefinitionDto {
            raw_workout_doc: event.workout_doc.clone(),
            intervals: parse_workout_doc(event.workout_doc.as_deref()),
        },
        actual_workout: None,
    }
}

fn map_activity_to_dto(activity: Activity) -> ActivityDto {
    ActivityDto {
        id: activity.id,
        start_date_local: activity.start_date_local,
        start_date: activity.start_date,
        name: activity.name,
        description: activity.description,
        activity_type: activity.activity_type,
        source: activity.source,
        external_id: activity.external_id,
        device_name: activity.device_name,
        distance_meters: activity.distance_meters,
        moving_time_seconds: activity.moving_time_seconds,
        elapsed_time_seconds: activity.elapsed_time_seconds,
        total_elevation_gain_meters: activity.total_elevation_gain_meters,
        average_speed_mps: activity.average_speed_mps,
        average_heart_rate_bpm: activity.average_heart_rate_bpm,
        average_cadence_rpm: activity.average_cadence_rpm,
        trainer: activity.trainer,
        commute: activity.commute,
        race: activity.race,
        has_heart_rate: activity.has_heart_rate,
        stream_types: activity.stream_types,
        tags: activity.tags,
        metrics: ActivityMetricsDto {
            training_stress_score: activity.metrics.training_stress_score,
            normalized_power_watts: activity.metrics.normalized_power_watts,
            intensity_factor: activity.metrics.intensity_factor,
            efficiency_factor: activity.metrics.efficiency_factor,
            variability_index: activity.metrics.variability_index,
            average_power_watts: activity.metrics.average_power_watts,
            ftp_watts: activity.metrics.ftp_watts,
            total_work_joules: activity.metrics.total_work_joules,
            calories: activity.metrics.calories,
            trimp: activity.metrics.trimp,
            power_load: activity.metrics.power_load,
            heart_rate_load: activity.metrics.heart_rate_load,
            pace_load: activity.metrics.pace_load,
            strain_score: activity.metrics.strain_score,
        },
        details: ActivityDetailsDto {
            intervals: activity
                .details
                .intervals
                .into_iter()
                .map(|interval| ActivityIntervalDto {
                    id: interval.id,
                    label: interval.label,
                    interval_type: interval.interval_type,
                    group_id: interval.group_id,
                    start_index: interval.start_index,
                    end_index: interval.end_index,
                    start_time_seconds: interval.start_time_seconds,
                    end_time_seconds: interval.end_time_seconds,
                    moving_time_seconds: interval.moving_time_seconds,
                    elapsed_time_seconds: interval.elapsed_time_seconds,
                    distance_meters: interval.distance_meters,
                    average_power_watts: interval.average_power_watts,
                    normalized_power_watts: interval.normalized_power_watts,
                    training_stress_score: interval.training_stress_score,
                    average_heart_rate_bpm: interval.average_heart_rate_bpm,
                    average_cadence_rpm: interval.average_cadence_rpm,
                    average_speed_mps: interval.average_speed_mps,
                    average_stride_meters: interval.average_stride_meters,
                    zone: interval.zone,
                })
                .collect(),
            interval_groups: activity
                .details
                .interval_groups
                .into_iter()
                .map(|group| ActivityIntervalGroupDto {
                    id: group.id,
                    count: group.count,
                    start_index: group.start_index,
                    moving_time_seconds: group.moving_time_seconds,
                    elapsed_time_seconds: group.elapsed_time_seconds,
                    distance_meters: group.distance_meters,
                    average_power_watts: group.average_power_watts,
                    normalized_power_watts: group.normalized_power_watts,
                    training_stress_score: group.training_stress_score,
                    average_heart_rate_bpm: group.average_heart_rate_bpm,
                    average_cadence_rpm: group.average_cadence_rpm,
                    average_speed_mps: group.average_speed_mps,
                    average_stride_meters: group.average_stride_meters,
                })
                .collect(),
            streams: activity
                .details
                .streams
                .into_iter()
                .map(map_activity_stream_to_dto)
                .collect(),
            interval_summary: activity.details.interval_summary,
            skyline_chart: activity.details.skyline_chart,
            power_zone_times: activity
                .details
                .power_zone_times
                .into_iter()
                .map(|zone| ActivityZoneTimeDto {
                    zone_id: zone.zone_id,
                    seconds: zone.seconds,
                })
                .collect(),
            heart_rate_zone_times: activity.details.heart_rate_zone_times,
            pace_zone_times: activity.details.pace_zone_times,
            gap_zone_times: activity.details.gap_zone_times,
        },
    }
}

fn map_activity_stream_to_dto(stream: ActivityStream) -> ActivityStreamDto {
    ActivityStreamDto {
        stream_type: stream.stream_type,
        name: stream.name,
        data: stream.data,
        data2: stream.data2,
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}

fn try_map_event_file_upload(
    file_upload: Option<EventFileUploadDto>,
) -> Result<Option<EventFileUpload>, StatusCode> {
    file_upload
        .map(EventFileUpload::try_from)
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, ()> {
    let clean: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    if clean.is_empty() || !clean.len().is_multiple_of(4) {
        return Err(());
    }

    let chunk_count = clean.len() / 4;
    for (chunk_index, chunk) in clean.as_bytes().chunks(4).enumerate() {
        let is_last = chunk_index + 1 == chunk_count;
        let padding_positions: Vec<usize> = chunk
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'=').then_some(index))
            .collect();

        if !padding_positions.is_empty() {
            if !is_last {
                return Err(());
            }

            match padding_positions.as_slice() {
                [3] | [2, 3] => {}
                _ => return Err(()),
            }
        }
    }

    let decoded = BASE64_STANDARD.decode(clean).map_err(|_| ())?;
    if decoded.len() > MAX_ACTIVITY_UPLOAD_BYTES {
        return Err(());
    }

    Ok(decoded)
}

fn normalize_optional_upload_field(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    })
}

fn parse_workout_doc(workout_doc: Option<&str>) -> Vec<IntervalDefinitionDto> {
    workout_doc
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| IntervalDefinitionDto {
            definition: line.to_string(),
        })
        .collect()
}

fn parse_category(category: &str) -> Option<EventCategory> {
    EventCategory::from_str(category).ok()
}

fn is_valid_date(date: &str) -> bool {
    let mut segments = date.split('-');
    let (Some(year), Some(month), Some(day), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };

    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.chars().all(|ch| ch.is_ascii_digit())
        || !month.chars().all(|ch| ch.is_ascii_digit())
        || !day.chars().all(|ch| ch.is_ascii_digit())
    {
        return false;
    }

    let Ok(year) = year.parse::<i32>() else {
        return false;
    };
    let Ok(month) = month.parse::<u32>() else {
        return false;
    };
    let Ok(day) = day.parse::<u32>() else {
        return false;
    };

    let is_leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year => 29,
        2 => 28,
        _ => return false,
    };

    (1..=max_day).contains(&day)
}

fn category_to_string(category: &EventCategory) -> String {
    category.as_str().to_string()
}
