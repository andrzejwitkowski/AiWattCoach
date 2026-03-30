use aiwattcoach::domain::intervals::{IntervalsConnectionError, IntervalsConnectionTester};

use super::app::BoxFuture;

#[derive(Clone)]
pub(crate) struct MockIntervalsConnectionTester {
    result: Result<(), IntervalsConnectionError>,
}

impl MockIntervalsConnectionTester {
    pub(crate) fn new(result: Result<(), IntervalsConnectionError>) -> Self {
        Self { result }
    }

    pub(crate) fn returning_ok() -> Self {
        Self::new(Ok(()))
    }

    pub(crate) fn returning_err(err: IntervalsConnectionError) -> Self {
        Self::new(Err(err))
    }
}

impl IntervalsConnectionTester for MockIntervalsConnectionTester {
    fn test_connection(
        &self,
        _api_key: &str,
        _athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>> {
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}
