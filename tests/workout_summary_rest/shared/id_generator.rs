use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone)]
pub(crate) struct TestIdGenerator {
    counter: Arc<AtomicUsize>,
}

impl Default for TestIdGenerator {
    fn default() -> Self {
        Self {
            counter: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl aiwattcoach::domain::identity::IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        let next = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{next}")
    }
}
