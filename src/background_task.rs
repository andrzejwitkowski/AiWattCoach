use tokio::{
    sync::watch,
    task::{JoinError, JoinHandle},
};
use tracing::warn;

#[derive(Debug)]
pub struct BackgroundTaskHandle {
    name: String,
    shutdown: watch::Sender<bool>,
    join_handle: Option<JoinHandle<()>>,
}

impl BackgroundTaskHandle {
    pub(crate) fn new(
        name: impl Into<String>,
        shutdown: watch::Sender<bool>,
        join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            name: name.into(),
            shutdown,
            join_handle: Some(join_handle),
        }
    }

    pub async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(join_handle) = self.join_handle.take() {
            log_background_join_result(&self.name, join_handle.await);
        }
    }

    pub async fn abort(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.abort();
            log_background_join_result(&self.name, join_handle.await);
        }
    }
}

impl Drop for BackgroundTaskHandle {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.abort();
        }
    }
}

fn log_background_join_result(name: &str, result: Result<(), JoinError>) {
    if let Err(error) = result {
        if error.is_cancelled() {
            return;
        }

        warn!(background_task = %name, %error, "background task exited unexpectedly");
    }
}
