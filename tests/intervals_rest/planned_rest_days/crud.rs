use axum::http::{Method, StatusCode};

use crate::fixtures::get_json;

use super::{
    fakes::RecordingPlannedRestDayService,
    support::{
        assert_status, authed_request, empty_body, json_authed_request,
        planned_rest_days_crud_test_app, send,
    },
};

#[tokio::test]
async fn create_list_get_update_delete_planned_rest_day_for_authenticated_user() {
    let service = RecordingPlannedRestDayService::default();
    let app = planned_rest_days_crud_test_app(service).await;

    let create = send(
        &app,
        json_authed_request(
            Method::POST,
            "/api/planned-rest-days",
            r#"{"startDate":"2026-12-24","endDate":"2026-12-26","title":"Holiday","note":"Family trip"}"#,
        ),
    )
    .await;
    assert_status(&create, StatusCode::CREATED);
    let created: serde_json::Value = get_json(create).await;
    let id = created
        .get("plannedRestDayId")
        .and_then(|value| value.as_str())
        .unwrap()
        .to_string();

    let list = send(
        &app,
        authed_request(
            Method::GET,
            "/api/planned-rest-days?oldest=2026-12-01&newest=2026-12-31",
            empty_body(),
        ),
    )
    .await;
    assert_status(&list, StatusCode::OK);
    let listed: serde_json::Value = get_json(list).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);

    let get = send(
        &app,
        authed_request(
            Method::GET,
            &format!("/api/planned-rest-days/{id}"),
            empty_body(),
        ),
    )
    .await;
    assert_status(&get, StatusCode::OK);

    let update = send(
        &app,
        json_authed_request(
            Method::PUT,
            &format!("/api/planned-rest-days/{id}"),
            r#"{"startDate":"2026-12-24","endDate":"2026-12-26","title":"Winter break","note":"Family trip"}"#,
        ),
    )
    .await;
    assert_status(&update, StatusCode::OK);
    let updated: serde_json::Value = get_json(update).await;
    assert_eq!(
        updated.get("title").and_then(|value| value.as_str()),
        Some("Winter break")
    );

    let delete = send(
        &app,
        authed_request(
            Method::DELETE,
            &format!("/api/planned-rest-days/{id}"),
            empty_body(),
        ),
    )
    .await;
    assert_status(&delete, StatusCode::NO_CONTENT);

    let missing = send(
        &app,
        authed_request(
            Method::GET,
            &format!("/api/planned-rest-days/{id}"),
            empty_body(),
        ),
    )
    .await;
    assert_status(&missing, StatusCode::NOT_FOUND);
}
