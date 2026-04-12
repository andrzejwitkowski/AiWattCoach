use std::sync::{Arc, Mutex};

use aiwattcoach::domain::intervals::{
    BoxFuture, IntervalsError, PestParserPocRepositoryPort, PestParserPocWorkoutRecord,
};

#[derive(Clone, Default)]
pub(crate) struct FakePestParserPocRepository {
    pub(crate) records: Arc<Mutex<Vec<PestParserPocWorkoutRecord>>>,
}

impl PestParserPocRepositoryPort for FakePestParserPocRepository {
    fn insert(&self, record: PestParserPocWorkoutRecord) -> BoxFuture<Result<(), IntervalsError>> {
        let records = self.records.clone();
        Box::pin(async move {
            records
                .lock()
                .expect("poc repo mutex poisoned")
                .push(record);
            Ok(())
        })
    }
}
