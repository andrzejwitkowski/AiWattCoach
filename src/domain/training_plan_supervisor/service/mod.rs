mod completion;
mod scheduler;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_support;
mod webhook;
mod webhook_flow;

use crate::domain::{identity::Clock, settings::UserSettingsUseCases};

use super::TrainingPlanSupervisorOperationRepository;

pub use scheduler::NoopTrainingPlanSupervisorScheduler;
pub use webhook::{
    GeminiTrainingPlanSupervisorWebhookService, TrainingPlanSupervisorWebhookUseCases,
};

#[derive(Clone)]
pub struct TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    repository: Repo,
    settings: Settings,
    clock: Time,
}

impl<Repo, Settings, Time> TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    pub fn new(repository: Repo, settings: Settings, clock: Time) -> Self {
        Self {
            repository,
            settings,
            clock,
        }
    }
}
