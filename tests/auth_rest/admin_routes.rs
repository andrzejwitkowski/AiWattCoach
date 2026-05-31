use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::util::ServiceExt;

use aiwattcoach::domain::task_scheduler::{
    AdminTaskSchedulerUseCases, BoxFuture as TaskBoxFuture, NewTask, RetryStrategy, ScheduledTask,
    TaskListFilter, TaskListPage, TaskSchedulerError, TaskStatus,
};

use crate::shared::{
    auth_test_app, auth_test_app_with_admin_task_scheduler, TestIdentityService,
    RESPONSE_LIMIT_BYTES,
};

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_requires_authentication() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_rejects_non_admin_user() {
    let app = auth_test_app(TestIdentityService {
        admin_cookie_role: aiwattcoach::domain::identity::Role::User,
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_rejects_stale_cookie_as_unauthorized() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=missing-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_returns_payload_for_admin() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
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
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["appName"], "AiWattCoach");
    assert_eq!(payload["mongoDatabase"], "aiwattcoach");
}

#[tokio::test(flavor = "current_thread")]
async fn admin_prompt_preview_post_workout_requires_admin() {
    let app = auth_test_app(TestIdentityService {
        admin_cookie_role: aiwattcoach::domain::identity::Role::User,
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/users/user-1/prompt-preview/post-workout?date=2026-05-01")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_prompt_preview_calendar_coach_returns_service_unavailable_without_wiring() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/users/user-1/prompt-preview/calendar-coach?date=2026-05-01")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_task_scheduler_list_requires_admin() {
    let app = auth_test_app_with_admin_task_scheduler(
        TestIdentityService {
            admin_cookie_role: aiwattcoach::domain::identity::Role::User,
            ..Default::default()
        },
        TestAdminTaskScheduler::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/task-scheduler/tasks")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_task_scheduler_list_requires_auth_even_when_service_is_missing() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/task-scheduler/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_task_scheduler_lists_tasks_for_admin() {
    let service =
        TestAdminTaskScheduler::new(vec![sample_task("task-2", 200, TaskStatus::Completed)]);
    let app =
        auth_test_app_with_admin_task_scheduler(TestIdentityService::default(), service).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/task-scheduler/tasks?limit=99&sortField=createdAt&sortDirection=desc")
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
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["limit"], 20);
    assert_eq!(payload["previousOffset"], Value::Null);
    assert_eq!(payload["nextOffset"], Value::Null);
    assert_eq!(payload["items"][0]["id"], "task-2");
    assert_eq!(payload["items"][0]["status"], "completed");
}

#[tokio::test(flavor = "current_thread")]
async fn admin_task_scheduler_retries_failed_task() {
    let service = TestAdminTaskScheduler::new(vec![sample_task("task-1", 100, TaskStatus::Failed)]);
    let app =
        auth_test_app_with_admin_task_scheduler(TestIdentityService::default(), service).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/task-scheduler/tasks/task-1/retry")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "queued");
}

#[tokio::test(flavor = "current_thread")]
async fn admin_task_scheduler_retry_rejects_completed_task() {
    let service =
        TestAdminTaskScheduler::new(vec![sample_task("task-1", 100, TaskStatus::Completed)]);
    let app =
        auth_test_app_with_admin_task_scheduler(TestIdentityService::default(), service).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/task-scheduler/tasks/task-1/retry")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[derive(Clone, Default)]
struct TestAdminTaskScheduler {
    tasks: std::sync::Arc<std::sync::Mutex<Vec<ScheduledTask>>>,
}

impl TestAdminTaskScheduler {
    fn new(tasks: Vec<ScheduledTask>) -> Self {
        Self {
            tasks: std::sync::Arc::new(std::sync::Mutex::new(tasks)),
        }
    }
}

impl AdminTaskSchedulerUseCases for TestAdminTaskScheduler {
    fn list_tasks(
        &self,
        filter: TaskListFilter,
    ) -> TaskBoxFuture<Result<TaskListPage, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap().clone();
            tasks.sort_by(|left, right| {
                right
                    .created_at_epoch_seconds
                    .cmp(&left.created_at_epoch_seconds)
            });
            let limit = filter.clamped_limit();
            let page = tasks
                .into_iter()
                .skip(filter.offset)
                .take(limit + 1)
                .collect::<Vec<_>>();
            Ok(TaskListPage {
                has_next_page: page.len() > limit,
                tasks: page.into_iter().take(limit).collect(),
            })
        })
    }

    fn get_task(
        &self,
        task_id: &str,
    ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move {
            Ok(tasks
                .lock()
                .unwrap()
                .iter()
                .find(|task| task.id == task_id)
                .cloned())
        })
    }

    fn retry_task(
        &self,
        task_id: &str,
    ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
            let Some(task) = tasks.iter_mut().find(|task| task.id == task_id) else {
                return Ok(None);
            };
            if !task.can_retry_manually() {
                return Ok(None);
            }
            task.status = TaskStatus::Queued;
            task.error_message = None;
            Ok(Some(task.clone()))
        })
    }
}

fn sample_task(id: &str, created_at_epoch_seconds: i64, status: TaskStatus) -> ScheduledTask {
    let mut task = ScheduledTask::new(
        NewTask {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            task_type: "summary".to_string(),
            payload: json!({ "task": id }),
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 3,
                delay_seconds: 30,
            },
            dedupe_key: format!("dedupe-{id}"),
            execution_timeout_seconds: 30,
            leader_only: false,
        },
        created_at_epoch_seconds,
    )
    .unwrap();
    task.error_message = matches!(status, TaskStatus::Failed | TaskStatus::TimedOut)
        .then(|| "task failed".to_string());
    task.finished_at_epoch_seconds = matches!(
        status,
        TaskStatus::Failed | TaskStatus::TimedOut | TaskStatus::Completed
    )
    .then_some(created_at_epoch_seconds + 10);
    task.status = status;
    task
}
