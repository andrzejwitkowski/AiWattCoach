use std::{
    error::Error,
    io::{Error as IoError, Write},
    sync::{Arc, Mutex},
};

#[cfg(unix)]
use crate::wait_for_sigterm;
use crate::{finish_server_shutdown, wait_for_ctrl_c};
use tokio::sync::Notify;
use tokio::time::{timeout, Duration};

#[derive(Clone, Default)]
struct SharedLogBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedLogBuffer {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().expect("log buffer mutex poisoned").clone())
            .expect("log buffer contained invalid utf-8")
    }
}

impl Write for SharedLogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("log buffer mutex poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
    type Writer = SharedLogBuffer;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ctrl_c_registration_error_logs_and_does_not_finish_shutdown_future() {
    let logs = SharedLogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_writer(logs.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);

    let result = timeout(
        Duration::from_millis(50),
        wait_for_ctrl_c(
            async { Err(IoError::other("boom")) },
            Arc::new(Notify::new()),
        ),
    )
    .await;

    assert!(result.is_err());
    let output = logs.contents();
    assert!(output.contains("Failed to listen for Ctrl+C"));
    assert!(output.contains("boom"));
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn sigterm_registration_error_logs_and_does_not_finish_shutdown_future() {
    let logs = SharedLogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_writer(logs.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);

    let result = timeout(
        Duration::from_millis(50),
        wait_for_sigterm(Err(IoError::other("boom")), Arc::new(Notify::new())),
    )
    .await;

    assert!(result.is_err());
    let output = logs.contents();
    assert!(output.contains("Failed to listen for SIGTERM"));
    assert!(output.contains("boom"));
}

#[test]
fn finish_server_shutdown_returns_ok_when_both_succeed() {
    assert!(finish_server_shutdown(Ok(()), Ok(())).is_ok());
}

#[test]
fn finish_server_shutdown_returns_telemetry_error_when_server_succeeds() {
    let error = finish_server_shutdown(Ok(()), Err(Box::new(IoError::other("telemetry boom"))))
        .expect_err("telemetry error should be returned");

    assert!(error.to_string().contains("telemetry boom"));
}

#[test]
fn finish_server_shutdown_combines_server_and_telemetry_errors() {
    let telemetry_error: Box<dyn Error + Send + Sync> = Box::new(IoError::other("telemetry boom"));
    let error = finish_server_shutdown(Err(IoError::other("server boom")), Err(telemetry_error))
        .expect_err("combined error should be returned");

    assert!(error.to_string().contains("server boom"));
    assert!(error.to_string().contains("telemetry boom"));
}
