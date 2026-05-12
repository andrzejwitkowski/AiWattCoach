use crate::domain::identity::Clock;

#[derive(Clone)]
pub struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_123
    }
}
