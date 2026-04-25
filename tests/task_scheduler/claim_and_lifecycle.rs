use aiwattcoach::domain::identity::Clock;

use crate::support::{service, task, TestClock};

#[tokio::test]
async fn claim_next_due_does_not_double_claim_same_task() {
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

    let claimed_by_worker_one = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("first claim should succeed");
    let claimed_by_worker_two = service
        .claim_next_due("worker-2", vec!["summary".to_string()], false, 30)
        .await
        .expect("second claim should succeed");

    assert_eq!(
        claimed_by_worker_one.expect("worker one should claim").id,
        "task-1"
    );
    assert!(claimed_by_worker_two.is_none());
}

#[tokio::test]
async fn leader_only_task_requires_leader_worker() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            true,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");

    let claimed_by_non_leader = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("non-leader claim should not error");
    let claimed_by_leader = service
        .claim_next_due("worker-2", vec!["summary".to_string()], true, 30)
        .await
        .expect("leader claim should not error");

    assert!(claimed_by_non_leader.is_none());
    assert_eq!(claimed_by_leader.expect("leader should claim").id, "task-1");
}
