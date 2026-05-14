use std::{sync::Arc, time::Duration};

use tokio::sync::Notify;

pub async fn wait_for_notify(
    notify: &Arc<Notify>,
    timeout_duration: Duration,
) -> Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(timeout_duration, notify.notified()).await
}
