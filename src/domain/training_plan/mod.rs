mod llm_output;
mod model;
mod ports;
mod prompt_guidance;
mod service;

pub(crate) use llm_output::should_retry_training_plan_llm_envelope_repair;
pub(crate) use service::parsing::split_into_day_blocks;

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
pub use prompt_guidance::{training_plan_output_grammar, training_plan_planning_guidelines};
pub use service::{
    training_plan_generate_task_handler, SchedulerBackedTrainingPlanService,
    TrainingPlanGenerationService, TrainingPlanUseCases,
};
