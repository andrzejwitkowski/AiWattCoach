use std::collections::BTreeSet;

use crate::domain::training_plan::{
    TrainingPlanError, TrainingPlanGenerationOperationRepository, TrainingPlanProjectionRepository,
};

use super::super::{
    ports::MesoCycleWindowPort, window::resolve_meso_window, MesoCycleError, MesoCycleWindow,
};

#[derive(Clone)]
pub struct TrainingPlanBackedMesoWindowPort<Ops, Projections> {
    operations: Ops,
    projections: Projections,
}

impl<Ops, Projections> TrainingPlanBackedMesoWindowPort<Ops, Projections> {
    pub fn new(operations: Ops, projections: Projections) -> Self {
        Self {
            operations,
            projections,
        }
    }

    fn map_training_plan_error(error: TrainingPlanError) -> MesoCycleError {
        match error {
            TrainingPlanError::Unavailable(message) => MesoCycleError::Unavailable(message),
            TrainingPlanError::Repository(message) => MesoCycleError::Repository(message),
            TrainingPlanError::Validation(message) => MesoCycleError::Validation(message),
        }
    }
}

impl<Ops, Projections> MesoCycleWindowPort for TrainingPlanBackedMesoWindowPort<Ops, Projections>
where
    Ops: TrainingPlanGenerationOperationRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
{
    fn resolve_window(
        &self,
        user_id: &str,
        today: &str,
    ) -> super::super::BoxFuture<Result<MesoCycleWindow, MesoCycleError>> {
        let operations = self.operations.clone();
        let projections = self.projections.clone();
        let user_id = user_id.to_string();
        let today = today.to_string();
        Box::pin(async move {
            let latest = operations
                .find_latest_completed_by_user_id(&user_id)
                .await
                .map_err(Self::map_training_plan_error)?;
            let Some(latest) = latest else {
                return resolve_meso_window(&today, None, None);
            };

            let projected = projections
                .find_active_by_operation_key(&latest.operation_key)
                .await
                .map_err(Self::map_training_plan_error)?;
            let ai_last = projected
                .iter()
                .map(|day| day.date.as_str())
                .max()
                .map(ToString::to_string);

            resolve_meso_window(&today, ai_last.as_deref(), Some(latest.operation_key))
        })
    }

    fn ai_coach_active_dates(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<BTreeSet<String>, MesoCycleError>> {
        let operations = self.operations.clone();
        let projections = self.projections.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let Some(latest) = operations
                .find_latest_completed_by_user_id(&user_id)
                .await
                .map_err(Self::map_training_plan_error)?
            else {
                return Ok(BTreeSet::new());
            };

            let projected = projections
                .find_active_by_operation_key(&latest.operation_key)
                .await
                .map_err(Self::map_training_plan_error)?;
            Ok(projected.into_iter().map(|day| day.date).collect())
        })
    }
}
