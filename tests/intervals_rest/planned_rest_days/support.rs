use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    response::Response,
    Router,
};
use tower::util::ServiceExt;

use aiwattcoach::domain::{
    calendar_labels::CalendarLabelSource, planned_rest_days::PlannedRestDayUseCases,
};

use crate::{
    app::{
        intervals_test_app_with_all_services, EmptyPlannedRestDayService,
        EmptyTrainingPlanProjectionRepository,
    },
    fixtures::session_cookie,
    identity_fakes::TestIdentityServiceWithSession,
    intervals_fakes::ScopedIntervalsService,
};

use super::fakes::{EmptyHiddenSource, EmptyPlannedRestLabelSource, StubRaceService};

pub(crate) async fn planned_rest_days_test_app(
    label_source: impl CalendarLabelSource + Clone + 'static,
    planned_rest_day_service: impl PlannedRestDayUseCases + Clone + 'static,
) -> Router {
    intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        label_source,
        EmptyHiddenSource,
        StubRaceService,
        planned_rest_day_service,
    )
    .await
}

pub(crate) async fn planned_rest_days_crud_test_app(
    planned_rest_day_service: impl PlannedRestDayUseCases + Clone + 'static,
) -> Router {
    planned_rest_days_test_app(EmptyPlannedRestLabelSource, planned_rest_day_service).await
}

pub(crate) async fn planned_rest_days_labels_test_app(
    label_source: impl CalendarLabelSource + Clone + 'static,
) -> Router {
    planned_rest_days_test_app(label_source, EmptyPlannedRestDayService).await
}

pub(crate) fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, session_cookie("session-1"))
        .body(body)
        .unwrap()
}

pub(crate) fn json_authed_request(method: Method, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, session_cookie("session-1"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub(crate) async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

pub(crate) fn empty_body() -> Body {
    Body::empty()
}

pub(crate) fn assert_status(response: &Response, expected: StatusCode) {
    assert_eq!(response.status(), expected);
}
