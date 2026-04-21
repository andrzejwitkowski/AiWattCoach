mod support;

use support::tracing_capture::{capture_tracing_logs_with_capture_id, is_capture_active};

#[tokio::test(flavor = "current_thread")]
async fn tracing_capture_does_not_accumulate_active_buffers_across_repeated_runs() {
    for attempt in 0..100 {
        let (value, logs, capture_id) = capture_tracing_logs_with_capture_id(|| async move {
            tracing::info!(attempt, "captured test log");
            attempt
        })
        .await;

        assert_eq!(value, attempt);
        assert!(logs.contains("captured test log"));
        assert!(!is_capture_active(&capture_id));
    }
}
