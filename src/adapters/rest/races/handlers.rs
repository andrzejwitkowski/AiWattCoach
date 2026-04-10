use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    config::AppState,
    domain::{
        intervals::DateRange,
        races::{CreateRace, RaceDiscipline, RacePriority, UpdateRace},
    },
};

use super::{
    dto::{ListRacesQuery, RacePath, UpsertRaceRequest},
    error::map_race_error,
    mapping::map_race_to_dto,
};

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

pub(in crate::adapters::rest) async fn list_races(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListRacesQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let race_service = match state.race_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !super::super::intervals::is_valid_date(&range.oldest)
        || !super::super::intervals::is_valid_date(&range.newest)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match race_service.list_races(&user_id, &range).await {
        Ok(races) => {
            Json(races.into_iter().map(map_race_to_dto).collect::<Vec<_>>()).into_response()
        }
        Err(error) => map_race_error(error),
    }
}

pub(in crate::adapters::rest) async fn get_race(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<RacePath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let race_service = match state.race_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match race_service.get_race(&user_id, &path.race_id).await {
        Ok(race) => Json(map_race_to_dto(race)).into_response(),
        Err(error) => map_race_error(error),
    }
}

pub(in crate::adapters::rest) async fn create_race(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpsertRaceRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let race_service = match state.race_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let request = match map_request(body) {
        Ok(request) => request,
        Err(status) => return status.into_response(),
    };

    match race_service.create_race(&user_id, request.into()).await {
        Ok(race) => (StatusCode::CREATED, Json(map_race_to_dto(race))).into_response(),
        Err(error) => map_race_error(error),
    }
}

pub(in crate::adapters::rest) async fn update_race(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<RacePath>,
    Json(body): Json<UpsertRaceRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let race_service = match state.race_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let request = match map_request(body) {
        Ok(request) => request,
        Err(status) => return status.into_response(),
    };

    match race_service
        .update_race(&user_id, &path.race_id, request)
        .await
    {
        Ok(race) => Json(map_race_to_dto(race)).into_response(),
        Err(error) => map_race_error(error),
    }
}

pub(in crate::adapters::rest) async fn delete_race(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<RacePath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let race_service = match state.race_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match race_service.delete_race(&user_id, &path.race_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_race_error(error),
    }
}

fn map_request(body: UpsertRaceRequest) -> Result<UpdateRace, StatusCode> {
    if !super::super::intervals::is_valid_date(&body.date) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let discipline = parse_discipline(&body.discipline).ok_or(StatusCode::BAD_REQUEST)?;
    let priority = parse_priority(&body.priority).ok_or(StatusCode::BAD_REQUEST)?;

    Ok(UpdateRace {
        date: body.date,
        name: body.name.trim().to_string(),
        distance_meters: body.distance_meters,
        discipline,
        priority,
    })
}

fn parse_discipline(value: &str) -> Option<RaceDiscipline> {
    match value.trim().to_ascii_lowercase().as_str() {
        "road" => Some(RaceDiscipline::Road),
        "mtb" => Some(RaceDiscipline::Mtb),
        "gravel" => Some(RaceDiscipline::Gravel),
        "cyclocross" => Some(RaceDiscipline::Cyclocross),
        _ => None,
    }
}

fn parse_priority(value: &str) -> Option<RacePriority> {
    match value.trim().to_ascii_uppercase().as_str() {
        "A" => Some(RacePriority::A),
        "B" => Some(RacePriority::B),
        "C" => Some(RacePriority::C),
        _ => None,
    }
}

impl From<UpdateRace> for CreateRace {
    fn from(value: UpdateRace) -> Self {
        Self {
            date: value.date,
            name: value.name,
            distance_meters: value.distance_meters,
            discipline: value.discipline,
            priority: value.priority,
        }
    }
}
