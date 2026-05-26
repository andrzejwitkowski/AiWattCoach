mod llm_output;
mod model;
mod ports;
mod service;

pub use llm_output::{
    parse_training_plan_llm_envelope, training_plan_llm_envelope_json_schema,
    TrainingPlanLlmEnvelope,
};
pub use model::{
    GeneratedTrainingPlan, TrainingPlanConversationMessage, TrainingPlanConversationRole,
    TrainingPlanDay, TrainingPlanError, TrainingPlanFailureState,
    TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation, TrainingPlanPhaseOutput,
    TrainingPlanPlanningContext, TrainingPlanProjectedDay, TrainingPlanReplacementResult,
    TrainingPlanSnapshot,
};
pub use ports::{
    BoxFuture, TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
    TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
    TrainingPlanToolLoopCheckpoint, TrainingPlanWorkoutSummaryPort,
};
pub use service::{
    training_plan_generate_task_handler, SchedulerBackedTrainingPlanService,
    TrainingPlanGenerationService, TrainingPlanUseCases,
};
