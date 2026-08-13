mod llm_output;
mod model;
mod planning_context;
mod ports;
mod prompt;
mod prompt_guidance;
mod race_projection_cleanup;
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
pub use planning_context::{map_workout_summary_to_planning_context, workout_recap_from_summary};
pub use ports::{
    BoxFuture, TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
    TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
    TrainingPlanToolLoopCheckpoint, TrainingPlanWorkoutSummaryPort, WorkoutPlanningLlmConfigPort,
};
pub use prompt::{
    assemble_training_plan_initial_window_request, latest_training_plan_user_message_epoch_seconds,
    planning_conversation_messages, training_plan_correction_system_prompt,
    training_plan_initial_window_system_prompt, training_plan_stable_context,
    training_plan_tool_context_today, TrainingPlanInitialWindowPromptInput,
    TRAINING_PLAN_INITIAL_WINDOW_USER_PROMPT,
};
pub use prompt_guidance::{
    training_plan_output_grammar, training_plan_planning_guidelines, TRAINING_PLAN_WINDOW_DAY_COUNT,
};
pub use race_projection_cleanup::{
    dates_to_supersede_for_race_date, is_race_placeholder_name, is_race_prep_name,
    orphan_race_dates_to_supersede, previous_calendar_date, projected_day_name,
    RaceProjectionCleanupService,
};
pub use service::{
    training_plan_generate_task_handler, SchedulerBackedTrainingPlanService,
    TrainingPlanGenerationService, TrainingPlanUseCases,
};
