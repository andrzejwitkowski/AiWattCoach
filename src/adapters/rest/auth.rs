use axum::{
    extract::{ConnectInfo, FromRequestParts, Query, State},
    http::request::Parts,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::time::{sleep, Duration, Instant};

use crate::{
    config::AppState,
    domain::identity::{is_valid_email, AppUser, GoogleLoginOutcome, IdentityError, Role},
};

use super::cookies::read_cookie;

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

#[derive(Deserialize)]
pub struct JoinWhitelistRequest {
    email: String,
}

#[derive(Serialize)]
struct JoinWhitelistResponse {
    success: bool,
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
    Unauthenticated {
        authenticated: bool,
    },
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
        Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
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
        Ok(GoogleLoginOutcome::SignedIn(result)) => {
            let mut response = Redirect::to(&result.redirect_to).into_response();
            let cookie = match build_session_cookie(
                &state.session_cookie_name,
                &state.session_cookie_same_site,
                &result.session.id,
                state.secure_session_cookie,
                state.session_ttl_hours,
            ) {
                Ok(cookie) => cookie,
                Err(status) => return status.into_response(),
            };
            response.headers_mut().insert(header::SET_COOKIE, cookie);
            response
        }
        Ok(GoogleLoginOutcome::PendingApproval { redirect_to }) => {
            Redirect::to(&redirect_to).into_response()
        }
        Err(IdentityError::InvalidLoginState) => StatusCode::BAD_REQUEST.into_response(),
        Err(IdentityError::Unauthenticated) => StatusCode::UNAUTHORIZED.into_response(),
        Err(IdentityError::EmailNotVerified) => StatusCode::UNAUTHORIZED.into_response(),
        Err(IdentityError::PendingApproval) => {
            Redirect::to("/?auth=pending-approval").into_response()
        }
        Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn current_user(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
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
        Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let clear_cookie = clear_session_cookie(
        &state.session_cookie_name,
        &state.session_cookie_same_site,
        state.secure_session_cookie,
    );
    let session_id = read_cookie(&headers, &state.session_cookie_name);

    let status = match (state.identity_service.clone(), session_id) {
        (Some(identity_service), Some(session_id)) => {
            match identity_service.logout(&session_id).await {
                Ok(()) => StatusCode::NO_CONTENT,
                Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
            }
        }
        (None, Some(_)) => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::NO_CONTENT,
    };

    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clear_cookie);
    response
}

pub async fn join_whitelist(
    State(state): State<AppState>,
    client_metadata: ClientMetadata,
    Json(payload): Json<JoinWhitelistRequest>,
) -> Response {
    let started_at = Instant::now();
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let client_ip = extract_client_ip(
        &client_metadata.headers,
        client_metadata.peer_addr,
        state.trust_proxy_headers,
    )
    .unwrap_or_else(|| "unknown".to_string());
    if !state.whitelist_rate_limiter.check(&client_ip) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    let trimmed_email = payload.email.trim();
    if !is_valid_email(trimmed_email) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let response = match identity_service
        .join_whitelist(trimmed_email.to_string())
        .await
    {
        Ok(_) => Json(JoinWhitelistResponse { success: true }).into_response(),
        Err(IdentityError::InvalidEmail) => StatusCode::BAD_REQUEST.into_response(),
        Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    sleep_until_minimum_response_time(started_at).await;
    response
}

fn extract_client_ip(
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
    trust_proxy_headers: bool,
) -> Option<String> {
    if trust_proxy_headers {
        if let Some(candidate) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_forwarded_header_ip)
        {
            return Some(candidate);
        }

        if let Some(candidate) = headers
            .get("x-real-ip")
            .and_then(|value| value.to_str().ok())
            .and_then(normalize_ip_candidate)
        {
            return Some(candidate);
        }

        if let Some(candidate) = headers
            .get("forwarded")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_forwarded_for)
        {
            return Some(candidate);
        }
    }

    peer_addr.map(|addr| addr.ip().to_string())
}

fn parse_forwarded_header_ip(value: &str) -> Option<String> {
    parse_forwarded_for(value).or_else(|| value.split(',').next().and_then(normalize_ip_candidate))
}

fn parse_forwarded_for(value: &str) -> Option<String> {
    let first = value.split(',').next()?.trim();

    for part in first.split(';') {
        let trimmed = part.trim();
        let Some((name, raw)) = trimmed.split_once('=') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("for") {
            continue;
        }
        return normalize_ip_candidate(raw.trim().trim_matches('"'));
    }

    None
}

fn normalize_ip_candidate(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();

    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix('[') {
        let (host, remainder) = rest.split_once(']')?;
        let parsed_host = host.parse::<std::net::IpAddr>().ok()?;

        if remainder.is_empty() {
            return Some(parsed_host.to_string());
        }

        if let Some(port) = remainder.strip_prefix(':') {
            if port.chars().all(|character| character.is_ascii_digit()) {
                return Some(parsed_host.to_string());
            }
        }

        return None;
    }

    if let Ok(ip) = trimmed.parse::<std::net::IpAddr>() {
        return Some(ip.to_string());
    }

    if let Some((host, port)) = trimmed.rsplit_once(':') {
        if !host.contains(':') && port.chars().all(|character| character.is_ascii_digit()) {
            return host
                .parse::<std::net::IpAddr>()
                .ok()
                .map(|ip| ip.to_string());
        }
    }

    None
}

async fn sleep_until_minimum_response_time(started_at: Instant) {
    // This is only a small abuse-resistance floor for common fast paths, not a strict side-channel guarantee.
    const MIN_WHITELIST_RESPONSE_TIME: Duration = Duration::from_millis(40);

    let elapsed = started_at.elapsed();
    if elapsed < MIN_WHITELIST_RESPONSE_TIME {
        sleep(MIN_WHITELIST_RESPONSE_TIME - elapsed).await;
    }
}

pub(super) struct ClientMetadata {
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
}

impl<S> FromRequestParts<S> for ClientMetadata
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let peer_addr = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0)
            .or_else(|| parts.extensions.get::<SocketAddr>().copied());

        Ok(Self {
            headers: parts.headers.clone(),
            peer_addr,
        })
    }
}

fn build_session_cookie(
    cookie_name: &str,
    same_site: &str,
    session_id: &str,
    secure: bool,
    session_ttl_hours: u64,
) -> Result<HeaderValue, StatusCode> {
    let max_age = session_ttl_hours
        .checked_mul(3600)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    HeaderValue::from_str(&format!(
        "{cookie_name}={session_id}; Path=/; HttpOnly; SameSite={}; Max-Age={max_age}{}",
        format_same_site_attribute(same_site),
        if secure { "; Secure" } else { "" }
    ))
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn clear_session_cookie(cookie_name: &str, same_site: &str, secure: bool) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{cookie_name}=; Path=/; Max-Age=0; HttpOnly; SameSite={}{}",
        format_same_site_attribute(same_site),
        if secure { "; Secure" } else { "" }
    ))
    .expect("validated cookie name should build a valid clearing cookie")
}

fn format_same_site_attribute(value: &str) -> &'static str {
    match value {
        "strict" => "Strict",
        "none" => "None",
        _ => "Lax",
    }
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
