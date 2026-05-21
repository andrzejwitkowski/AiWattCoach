use crate::domain::identity::Clock;

#[derive(Clone, Copy)]
pub(crate) struct FixedClock {
    pub(crate) now_epoch_seconds: i64,
}

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.now_epoch_seconds
    }
}
