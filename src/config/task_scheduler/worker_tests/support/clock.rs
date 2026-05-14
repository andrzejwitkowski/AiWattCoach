use std::sync::{Arc, Mutex};

use crate::domain::identity::Clock;

#[derive(Clone)]
pub struct TestClock {
    now_epoch_seconds: Arc<Mutex<i64>>,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::new(1_700_000_000)
    }
}

impl TestClock {
    pub fn new(now_epoch_seconds: i64) -> Self {
        Self {
            now_epoch_seconds: Arc::new(Mutex::new(now_epoch_seconds)),
        }
    }
}

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        *self.now_epoch_seconds.lock().expect("clock mutex poisoned")
    }
}
