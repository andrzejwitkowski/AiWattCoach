use std::sync::{Arc, Mutex};

use crate::domain::external_sync::{ExternalProvider, ProviderPollState, ProviderPollStream};

use super::{support::*, ProviderPollingService};

#[tokio::test]
async fn poll_due_once_persists_attempt_before_calling_intervals() {
    let shared_states = Arc::new(Mutex::new(vec![ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_699_999_900,
    )]));
    let poll_states = RecordingProviderPollStateRepository {
        states: shared_states.clone(),
    };
    let service = ProviderPollingService::new(
        AssertingIntervalsApi::new(shared_states),
        FakeIntervalsSettings,
        poll_states,
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7);

    service.poll_due_once().await.unwrap();
}
