use std::sync::{Arc, Mutex};

use aiwattcoach::domain::identity::Clock;

#[derive(Clone)]
pub struct TestClock {
    now: Arc<Mutex<i64>>,
}

impl TestClock {
    pub fn new(now_epoch_seconds: i64) -> Self {
        Self {
            now: Arc::new(Mutex::new(now_epoch_seconds)),
        }
    }

    pub fn set_now(&self, now_epoch_seconds: i64) {
        *self.now.lock().expect("clock mutex poisoned") = now_epoch_seconds;
    }
}

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        *self.now.lock().expect("clock mutex poisoned")
    }
}
