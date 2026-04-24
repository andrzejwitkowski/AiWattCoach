mod model;
mod ports;
mod service;

pub use model::{
    GeneratedTrainingPlan, TrainingPlanConversationMessage, TrainingPlanConversationRole,
    TrainingPlanDay, TrainingPlanError, TrainingPlanFailureState,
    TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
    TrainingPlanPlanningContext, TrainingPlanProjectedDay, TrainingPlanReplacementResult,
    TrainingPlanSnapshot,
};
pub use ports::{
    BoxFuture, TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
    TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
    TrainingPlanWorkoutSummaryPort,
};
pub use service::{TrainingPlanGenerationService, TrainingPlanUseCases};
