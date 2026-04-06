use super::*;

#[derive(Clone)]
pub(crate) struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestIdGenerator {
    next_id: Arc<AtomicUsize>,
}

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        format!("{prefix}-{next_id}")
    }
}
