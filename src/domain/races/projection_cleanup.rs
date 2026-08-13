use crate::domain::{
    identity::Clock,
    races::{BoxFuture, RaceError},
    training_plan::{RaceProjectionCleanupService, TrainingPlanProjectionRepository},
};

pub trait RaceProjectionCleanupPort: Clone + Send + Sync + 'static {
    fn supersede_for_deleted_race_date(
        &self,
        user_id: &str,
        race_date: &str,
    ) -> BoxFuture<Result<Option<(String, String)>, RaceError>>;
}

#[derive(Clone, Default)]
pub struct NoopRaceProjectionCleanup;

impl RaceProjectionCleanupPort for NoopRaceProjectionCleanup {
    fn supersede_for_deleted_race_date(
        &self,
        _user_id: &str,
        _race_date: &str,
    ) -> BoxFuture<Result<Option<(String, String)>, RaceError>> {
        Box::pin(async { Ok(None) })
    }
}

impl<Projections, Time> RaceProjectionCleanupPort
    for RaceProjectionCleanupService<Projections, Time>
where
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    fn supersede_for_deleted_race_date(
        &self,
        user_id: &str,
        race_date: &str,
    ) -> BoxFuture<Result<Option<(String, String)>, RaceError>> {
        let fut =
            RaceProjectionCleanupService::supersede_for_deleted_race_date(self, user_id, race_date);
        Box::pin(async move {
            fut.await.map_err(|error| {
                RaceError::Internal(format!("race projection cleanup failed: {error}"))
            })
        })
    }
}
