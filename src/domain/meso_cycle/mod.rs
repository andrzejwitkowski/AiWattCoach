mod model;
mod parsing;
mod ports;
mod prompt;
mod service;
mod window;

pub use model::{
    MesoCycleCalendarDay, MesoCycleDay, MesoCycleError, MesoCycleFailureState,
    MesoCycleGenerationClaimResult, MesoCycleGenerationOperation, MesoCycleOverlapStatus,
    MesoCyclePhaseOutput, MesoCycleProjectedDay, MesoCycleStatus, MesoCycleWindow,
    MESO_CYCLE_RECENT_DAY_COUNT, MESO_CYCLE_WINDOW_DAY_COUNT,
};
pub use parsing::parse_meso_plan_window;
pub use ports::{
    BoxFuture, MesoCycleGenerationOperationRepository, MesoCycleGenerator, MesoCycleLlmConfigPort,
    MesoCycleProjectionRepository, MesoCycleToolLoopCheckpoint, MesoCycleWindowPort,
};
pub use prompt::{
    assemble_meso_cycle_coach_request, meso_cycle_system_prompt, MesoCycleCoachPromptBundle,
    MesoCycleCoachPromptInput,
};
pub use service::{
    meso_cycle_generate_task_handler, MesoCycleService, MesoCycleServiceExecutor,
    MesoCycleUseCases, SchedulerBackedMesoCycleService, TrainingPlanBackedMesoWindowPort,
    GENERATION_ALREADY_PENDING_MESSAGE, MESO_CYCLE_STALE_PENDING_TIMEOUT_SECONDS,
};
pub use window::resolve_meso_window;
