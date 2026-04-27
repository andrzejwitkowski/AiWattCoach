use std::time::Duration;

use aiwattcoach::domain::{
    identity::Clock,
    task_scheduler::{BoxFuture, ResultTaskHandler, ScheduledTask, TaskSchedulerError},
};

use crate::support::{service, task, TestClock};

struct TimeoutResultHandler;

impl ResultTaskHandler for TimeoutResultHandler {
    type Completed = ();
    type Output = ();
    type Error = String;

    fn task_disappeared(&self, task_id: &str) -> Self::Error {
        format!("task disappeared: {task_id}")
    }

    fn task_timed_out(&self, task_id: &str) -> Self::Error {
        format!("task timed out: {task_id}")
    }

    fn parse_completed(&self, _task: &ScheduledTask) -> Result<Self::Completed, Self::Error> {
        Ok(())
    }

    fn parse_failed(&self, _task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
        Ok("task failed unexpectedly".to_string())
    }

    fn finish(&self, _completed: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>> {
        Box::pin(async { Ok(()) })
    }
}

fn map_scheduler_error(error: TaskSchedulerError) -> String {
    error.to_string()
}

#[tokio::test]
async fn timeout_sweep_keeps_running_task_when_owner_reports_it_active() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);
    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");

    let timed_out = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out, 0);
    assert_eq!(
        task.status,
        aiwattcoach::domain::task_scheduler::TaskStatus::Running
    );
}

#[tokio::test]
async fn timeout_sweep_notifies_result_waiters_across_scheduler_clones() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");
    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");

    let waiting_service = service.clone();
    let waiter = tokio::spawn(async move {
        waiting_service
            .wait_for_result_task("task-1", map_scheduler_error, TimeoutResultHandler)
            .await
    });

    tokio::task::yield_now().await;
    clock.set_now(160);

    let timed_out = service
        .clone()
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");

    let wait_result = tokio::time::timeout(Duration::from_secs(2), waiter)
        .await
        .expect("waiter should resolve after timeout sweep")
        .expect("waiter join should succeed");

    assert_eq!(timed_out, 1);
    assert_eq!(wait_result, Err("task timed out: task-1".to_string()));
}

#[tokio::test]
async fn timeout_sweep_marks_task_timed_out_when_owner_is_stale() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");
    clock.set_now(160);

    let timed_out = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out, 1);
    assert_eq!(
        task.status,
        aiwattcoach::domain::task_scheduler::TaskStatus::TimedOut
    );
    assert!(task.claimed_by.is_none());
}

#[tokio::test]
async fn timeout_sweep_recovers_task_when_worker_disappears() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);

    let recovered = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(recovered, 1);
    assert_eq!(
        task.status,
        aiwattcoach::domain::task_scheduler::TaskStatus::RetryScheduled
    );
    assert!(task.claimed_by.is_none());
}

#[tokio::test]
async fn timeout_sweep_recovers_task_when_worker_restarts_without_active_claim() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);
    service
        .touch_worker_heartbeat("worker-1", false, vec!["summary".to_string()])
        .await
        .expect("worker heartbeat should succeed");

    let recovered = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let reclaimed = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("reclaim should succeed")
        .expect("restarted worker should reclaim recovered task");

    assert_eq!(recovered, 1);
    assert_eq!(reclaimed.id, "task-1");
    assert_eq!(reclaimed.claimed_by.as_deref(), Some("worker-1"));
}

#[tokio::test]
async fn touch_worker_heartbeat_clears_stale_active_task_ids_before_recovery() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");
    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");

    clock.set_now(160);
    service
        .touch_worker_heartbeat("worker-1", false, vec!["summary".to_string()])
        .await
        .expect("touch heartbeat should succeed");

    let worker = service
        .heartbeat_worker("worker-1", false, vec!["summary".to_string()], Vec::new())
        .await
        .expect("worker heartbeat should reload worker");
    assert!(worker.active_task_ids.is_empty());

    let recovered = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let reclaimed = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("claim should succeed")
        .expect("task should be reclaimed after stale active ids are cleared");

    assert_eq!(recovered, 1);
    assert_eq!(reclaimed.id, "task-1");
}

#[tokio::test]
async fn timeout_sweep_recovers_when_worker_restart_drops_active_claim_even_with_fresh_task_heartbeat(
) {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");
    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");

    clock.set_now(160);
    service
        .heartbeat_task("task-1", "worker-1", 30)
        .await
        .expect("task heartbeat should succeed")
        .expect("task should accept heartbeat");
    service
        .touch_worker_heartbeat("worker-1", false, vec!["summary".to_string()])
        .await
        .expect("touch heartbeat should succeed");

    let recovered = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(recovered, 1);
    assert_eq!(
        task.status,
        aiwattcoach::domain::task_scheduler::TaskStatus::RetryScheduled
    );
}

#[tokio::test]
async fn timeout_sweep_keeps_running_task_when_task_heartbeat_is_fresh() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);
    service
        .heartbeat_task("task-1", "worker-1", 30)
        .await
        .expect("task heartbeat should succeed")
        .expect("task heartbeat should keep task alive");

    let timed_out = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out, 0);
    assert_eq!(
        task.status,
        aiwattcoach::domain::task_scheduler::TaskStatus::Running
    );
}
