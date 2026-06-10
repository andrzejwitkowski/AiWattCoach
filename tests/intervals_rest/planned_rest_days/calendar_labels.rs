use aiwattcoach::domain::planned_rest_days::PlannedRestDay;
use axum::http::{Method, StatusCode};

use crate::fixtures::get_json;

use super::{
    fakes::PlannedRestLabelSource,
    fixtures::sample_planned_rest_day,
    support::{assert_status, authed_request, empty_body, planned_rest_days_labels_test_app, send},
};

#[tokio::test]
async fn list_calendar_labels_does_not_fill_gap_between_separate_planned_rest_ranges() {
    let app = planned_rest_days_labels_test_app(PlannedRestLabelSource::with_entries(vec![
        PlannedRestDay::new(
            "prd-july".to_string(),
            "user-1".to_string(),
            "2026-07-01".to_string(),
            "2026-07-07".to_string(),
            None,
            None,
            1,
            1,
        )
        .unwrap(),
        PlannedRestDay::new(
            "prd-august".to_string(),
            "user-1".to_string(),
            "2026-08-01".to_string(),
            "2026-08-08".to_string(),
            None,
            None,
            1,
            1,
        )
        .unwrap(),
    ]))
    .await;

    let response = send(
        &app,
        authed_request(
            Method::GET,
            "/api/calendar/labels?oldest=2026-07-01&newest=2026-08-31",
            empty_body(),
        ),
    )
    .await;

    assert_status(&response, StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    let labels_by_date = body.get("labelsByDate").unwrap();

    for date in ["2026-07-01", "2026-07-07", "2026-08-01", "2026-08-08"] {
        assert!(
            labels_by_date.get(date).is_some(),
            "expected planned rest label on {date}"
        );
    }

    for date in ["2026-07-08", "2026-07-15", "2026-07-31"] {
        assert!(
            labels_by_date.get(date).is_none(),
            "did not expect planned rest label on gap day {date}"
        );
    }
}

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
