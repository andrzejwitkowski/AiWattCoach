use axum::http::{Method, StatusCode};

use crate::fixtures::get_json;

use super::{
    fakes::PlannedRestLabelSource,
    fixtures::sample_planned_rest_day,
    support::{assert_status, authed_request, empty_body, planned_rest_days_labels_test_app, send},
};

#[tokio::test]
async fn list_calendar_labels_returns_planned_rest_day_labels_for_each_day() {
    let app = planned_rest_days_labels_test_app(PlannedRestLabelSource::with_entries(vec![
        sample_planned_rest_day(),
    ]))
    .await;

    let response = send(
        &app,
        authed_request(
            Method::GET,
            "/api/calendar/labels?oldest=2026-12-01&newest=2026-12-31",
            empty_body(),
        ),
    )
    .await;

    assert_status(&response, StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    for date in ["2026-12-24", "2026-12-25", "2026-12-26"] {
        assert_eq!(
            body.get("labelsByDate")
                .and_then(|value| value.get(date))
                .and_then(|value| value.get("planned_rest_day:prd-1"))
                .and_then(|value| value.get("kind"))
                .and_then(|value| value.as_str()),
            Some("planned_rest_day")
        );
    }
}
