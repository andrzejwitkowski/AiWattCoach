use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{config::AppState, domain::identity::{AppUser, Role}};

#[derive(Deserialize)]
pub struct StartGoogleLoginQuery {
    #[serde(rename = "returnTo")]
    return_to: Option<String>,
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    state: String,
    code: String,
}

#[derive(Serialize)]
struct CurrentUserDto {
    id: String,
    email: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "avatarUrl")]
    avatar_url: Option<String>,
    roles: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AuthMeResponse {
    Unauthenticated { authenticated: bool },
    Authenticated {
        authenticated: bool,
        user: CurrentUserDto,
    },
}

pub async fn start_google_login(
    State(state): State<AppState>,
    Query(query): Query<StartGoogleLoginQuery>,
) -> Response {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match identity_service.begin_google_login(query.return_to).await {
        Ok(login_start) => Redirect::temporary(&login_start.redirect_url).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn finish_google_login(
    State(state): State<AppState>,
    Query(query): Query<GoogleCallbackQuery>,
) -> Response {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match identity_service
        .handle_google_callback(&query.state, &query.code)
        .await
    {
        Ok(result) => {
            let mut response = Redirect::to(&result.redirect_to).into_response();
            response.headers_mut().insert(
                header::SET_COOKIE,
                build_session_cookie(
                    &state.session_cookie_name,
                    &result.session.id,
                    state.secure_session_cookie,
                ),
            );
            response
        }
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

pub async fn current_user(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity_service) = state.identity_service.clone() else {
        return Json(AuthMeResponse::Unauthenticated {
            authenticated: false,
        })
        .into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return Json(AuthMeResponse::Unauthenticated {
            authenticated: false,
        })
        .into_response();
    };

    match identity_service.get_current_user(&session_id).await {
        Ok(Some(user)) => Json(AuthMeResponse::Authenticated {
            authenticated: true,
            user: map_current_user(user),
        })
        .into_response(),
        Ok(None) => Json(AuthMeResponse::Unauthenticated {
            authenticated: false,
        })
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(identity_service) = state.identity_service.clone() {
        if let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) {
            let _ = identity_service.logout(&session_id).await;
        }
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        clear_session_cookie(&state.session_cookie_name, state.secure_session_cookie),
    );
    response
}

fn map_current_user(user: AppUser) -> CurrentUserDto {
    CurrentUserDto {
        id: user.id,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        roles: user
            .roles
            .into_iter()
            .map(|role| match role {
                Role::User => "user",
                Role::Admin => "admin",
            })
            .collect(),
    }
}

fn read_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    cookie_header.split(';').find_map(|entry| {
        let trimmed = entry.trim();
        let (name, value) = trimmed.split_once('=')?;

        if name == cookie_name {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn build_session_cookie(cookie_name: &str, session_id: &str, secure: bool) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{cookie_name}={session_id}; Path=/; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    ))
    .unwrap()
}

fn clear_session_cookie(cookie_name: &str, secure: bool) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{cookie_name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    ))
    .unwrap()
}
