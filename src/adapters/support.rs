use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::domain::identity::{Clock, IdGenerator};

#[derive(Clone)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_epoch_seconds(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

#[derive(Clone)]
pub struct UuidIdGenerator;

impl IdGenerator for UuidIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", Uuid::new_v4().simple())
    }
}
