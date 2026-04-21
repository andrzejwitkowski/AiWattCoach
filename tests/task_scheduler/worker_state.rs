use aiwattcoach::domain::identity::Clock;

use crate::support::{service, task, TestClock};

#[tokio::test]
async fn worker_heartbeat_keeps_leader_flag_when_active_tasks_change() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
        .await
        .expect("worker heartbeat should succeed");
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
        .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
        .await
        .expect("adding active task should succeed");
    let running_worker = service
        .heartbeat_worker(
            "worker-1",
            true,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker reload should succeed");

    assert!(running_worker.is_leader);
    assert_eq!(running_worker.active_task_ids, vec!["task-1".to_string()]);

    service
        .remove_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
        .await
        .expect("removing active task should succeed");
    let idle_worker = service
        .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
        .await
        .expect("worker reload should succeed");

    assert!(idle_worker.is_leader);
    assert!(idle_worker.active_task_ids.is_empty());
}
