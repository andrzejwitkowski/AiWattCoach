mod support;

use support::tracing_capture::{active_log_buffer_count, capture_tracing_logs};

#[tokio::test(flavor = "current_thread")]
async fn tracing_capture_does_not_accumulate_active_buffers_across_repeated_runs() {
    for attempt in 0..100 {
        let (value, logs) = capture_tracing_logs(|| async move {
            tracing::info!(attempt, "captured test log");
            attempt
        })
        .await;

        assert_eq!(value, attempt);
        assert!(logs.contains("captured test log"));
        assert_eq!(active_log_buffer_count(), 0);
    }
}
