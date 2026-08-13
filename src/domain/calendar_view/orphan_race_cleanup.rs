use std::collections::BTreeSet;

use crate::domain::{
    calendar_view::{BoxFuture, CalendarEntryViewError},
    identity::Clock,
    training_plan::{RaceProjectionCleanupService, TrainingPlanProjectionRepository},
};

pub trait OrphanRaceProjectionCleanupPort: Clone + Send + Sync + 'static {
    fn supersede_orphan_race_projections(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        race_dates_present: &BTreeSet<String>,
    ) -> BoxFuture<Result<(), CalendarEntryViewError>>;
}

#[derive(Clone, Default)]
pub struct NoopOrphanRaceProjectionCleanup;

impl OrphanRaceProjectionCleanupPort for NoopOrphanRaceProjectionCleanup {
    fn supersede_orphan_race_projections(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
        _race_dates_present: &BTreeSet<String>,
    ) -> BoxFuture<Result<(), CalendarEntryViewError>> {
        Box::pin(async { Ok(()) })
    }
}

impl<Projections, Time> OrphanRaceProjectionCleanupPort
    for RaceProjectionCleanupService<Projections, Time>
where
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    fn supersede_orphan_race_projections(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        race_dates_present: &BTreeSet<String>,
    ) -> BoxFuture<Result<(), CalendarEntryViewError>> {
        let fut = RaceProjectionCleanupService::supersede_orphan_race_projections(
            self,
            user_id,
            oldest,
            newest,
            race_dates_present,
        );
        Box::pin(async move {
            fut.await.map_err(|error| {
                CalendarEntryViewError::Repository(format!(
                    "orphan race projection cleanup failed: {error}"
                ))
            })
        })
    }
}
