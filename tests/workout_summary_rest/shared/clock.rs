#[derive(Clone)]
pub(crate) struct TestClock;

impl aiwattcoach::domain::identity::Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}
