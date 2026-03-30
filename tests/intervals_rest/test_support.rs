use std::{future::Future, pin::Pin};

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub(crate) fn assert_log_entry_contains(logs: &str, expected_fragments: &[&str]) {
    let matched = logs.lines().any(|line| {
        expected_fragments
            .iter()
            .all(|fragment| line.contains(fragment))
    });

    assert!(
        matched,
        "expected one log entry to contain {expected_fragments:?}, logs were: {logs}"
    );
}
