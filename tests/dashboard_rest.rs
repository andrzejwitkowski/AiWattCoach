#[path = "dashboard_rest/support.rs"]
mod dashboard_rest_support;

use aiwattcoach::domain::identity::{
    AppUser, AuthSession, GoogleLoginOutcome, GoogleLoginStart, GoogleLoginSuccess, IdentityError,
    IdentityUseCases, Role, WhitelistEntry,
};
use aiwattcoach::domain::training_load::TrainingLoadDailySnapshotRepository;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use dashboard_rest_support::{dashboard_test_app, sample_snapshot};

const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

#[derive(Clone, Default)]
struct TestIdentityServiceWithSession {
    sessions: std::collections::HashMap<String, (String, String)>,
}

impl TestIdentityServiceWithSession {
    fn with_sessions(sessions: Vec<(&str, &str, &str)>) -> Self {
        Self {
            sessions: sessions
                .into_iter()
                .map(|(session_id, user_id, email)| {
                    (
                        session_id.to_string(),
                        (user_id.to_string(), email.to_string()),
                    )
                })
                .collect(),
        }
    }
}

impl IdentityUseCases for TestIdentityServiceWithSession {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
        Box::pin(async {
            Ok(GoogleLoginStart {
                state: "state-1".to_string(),
                redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
                    .to_string(),
            })
        })
    }

    fn join_whitelist(&self, email: String) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
        Box::pin(async move { Ok(WhitelistEntry::new(email, false, 100, 100)) })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>> {
        Box::pin(async {
            Ok(GoogleLoginOutcome::SignedIn(Box::new(GoogleLoginSuccess {
                user: AppUser::new(
                    "user-1".to_string(),
                    "google-subject-1".to_string(),
                    "athlete@example.com".to_string(),
                    vec![Role::User],
                    Some("Test User".to_string()),
                    None,
                    true,
                ),
                session: AuthSession::new(
                    "session-1".to_string(),
                    "user-1".to_string(),
                    999999,
                    100,
                ),
                redirect_to: "/app".to_string(),
            })))
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let sessions = self.sessions.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            Ok(sessions.get(&session_id).map(|(user_id, email)| {
                AppUser::new(
                    user_id.clone(),
                    format!("google-subject-{user_id}"),
                    email.clone(),
                    vec![Role::User],
                    Some("Test User".to_string()),
                    None,
                    true,
                )
            }))
        })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, _session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        Box::pin(async { Err(IdentityError::Forbidden) })
    }
}

#[derive(Clone, Default)]
struct InMemoryTrainingLoadDailySnapshotRepository {
    snapshots: std::sync::Arc<
        std::sync::Mutex<Vec<aiwattcoach::domain::training_load::TrainingLoadDailySnapshot>>,
    >,
}

impl TrainingLoadDailySnapshotRepository for InMemoryTrainingLoadDailySnapshotRepository {
    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &aiwattcoach::domain::training_load::TrainingLoadSnapshotRange,
    ) -> aiwattcoach::domain::training_load::BoxFuture<
        Result<
            Vec<aiwattcoach::domain::training_load::TrainingLoadDailySnapshot>,
            aiwattcoach::domain::training_load::TrainingLoadError,
        >,
    > {
        let snapshots = self.snapshots.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            let mut values = snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|snapshot| snapshot.user_id == user_id)
                .filter(|snapshot| snapshot.date >= oldest && snapshot.date <= newest)
                .cloned()
                .collect::<Vec<_>>();
            values.sort_by(|left, right| left.date.cmp(&right.date));
            Ok(values)
        })
    }

    fn find_oldest_date_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::training_load::BoxFuture<
        Result<Option<String>, aiwattcoach::domain::training_load::TrainingLoadError>,
    > {
        let snapshots = self.snapshots.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|snapshot| snapshot.user_id == user_id)
                .map(|snapshot| snapshot.date.clone())
                .min())
        })
    }

    fn upsert(
        &self,
        snapshot: aiwattcoach::domain::training_load::TrainingLoadDailySnapshot,
    ) -> aiwattcoach::domain::training_load::BoxFuture<
        Result<
            aiwattcoach::domain::training_load::TrainingLoadDailySnapshot,
            aiwattcoach::domain::training_load::TrainingLoadError,
        >,
    > {
        let snapshots = self.snapshots.clone();
        Box::pin(async move {
            let mut snapshots = snapshots.lock().unwrap();
            snapshots.retain(|existing| {
                !(existing.user_id == snapshot.user_id && existing.date == snapshot.date)
            });
            snapshots.push(snapshot.clone());
            Ok(snapshot)
        })
    }

    fn delete_by_user_id_from_date(
        &self,
        user_id: &str,
        from_date: &str,
    ) -> aiwattcoach::domain::training_load::BoxFuture<
        Result<(), aiwattcoach::domain::training_load::TrainingLoadError>,
    > {
        let snapshots = self.snapshots.clone();
        let user_id = user_id.to_string();
        let from_date = from_date.to_string();
        Box::pin(async move {
            snapshots
                .lock()
                .unwrap()
                .retain(|existing| !(existing.user_id == user_id && existing.date >= from_date));
            Ok(())
        })
    }
}

#[tokio::test]
async fn training_load_dashboard_returns_points_for_authenticated_user_only() {
    let snapshots = InMemoryTrainingLoadDailySnapshotRepository::default();
    snapshots
        .upsert(sample_snapshot(
            "user-1",
            "2026-04-01",
            Some(44),
            Some(20.0),
            Some(30.0),
            Some(-10.0),
        ))
        .await
        .unwrap();
    snapshots
        .upsert(sample_snapshot(
            "user-1",
            "2026-04-18",
            Some(97),
            Some(29.9),
            Some(53.4),
            Some(-23.5),
        ))
        .await
        .unwrap();
    snapshots
        .upsert(sample_snapshot(
            "user-2",
            "2026-04-18",
            Some(11),
            Some(10.0),
            Some(12.0),
            Some(-2.0),
        ))
        .await
        .unwrap();

    let app = dashboard_test_app(
        TestIdentityServiceWithSession::with_sessions(vec![(
            "session-1",
            "user-1",
            "athlete@example.com",
        )]),
        snapshots,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard/training-load?range=all-time")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["windowStart"], "2026-04-01");
    assert_eq!(json["points"].as_array().unwrap().len(), 2);
    assert_eq!(json["summary"]["currentCtl"], 29.9);
}

#[tokio::test]
async fn training_load_dashboard_rejects_unknown_range() {
    let app = dashboard_test_app(
        TestIdentityServiceWithSession::with_sessions(vec![(
            "session-1",
            "user-1",
            "athlete@example.com",
        )]),
        InMemoryTrainingLoadDailySnapshotRepository::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard/training-load?range=nope")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn training_load_dashboard_returns_empty_payload_when_user_has_no_snapshots() {
    let app = dashboard_test_app(
        TestIdentityServiceWithSession::with_sessions(vec![(
            "session-1",
            "user-1",
            "athlete@example.com",
        )]),
        InMemoryTrainingLoadDailySnapshotRepository::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard/training-load?range=all-time")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["hasTrainingLoad"], false);
    assert_eq!(json["points"].as_array().unwrap().len(), 0);
}
