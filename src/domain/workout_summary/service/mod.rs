use std::sync::Arc;

use crate::domain::{
    athlete_summary::AthleteSummaryUseCases,
    identity::{Clock, IdGenerator},
    settings::UserSettingsUseCases,
    training_plan::TrainingPlanUseCases,
};

use super::{
    save_completion_port::SaveWorkflowCompletionPort, validate_message_content, validate_rpe,
    BoxFuture, CoachReply, CoachReplyOperation, CoachReplyOperationRepository, CompletedCoachReply,
    ConversationMessage, MessageRole, PendingCoachReplyCheckpoint, PersistedUserMessage,
    SendMessageResult, WorkoutCoach, WorkoutRecap, WorkoutSummary, WorkoutSummaryError,
    WorkoutSummaryRepository,
};

mod internals;
mod scheduler;
mod use_cases;

#[cfg(test)]
mod tests;

const POST_PROVIDER_WRITE_ATTEMPTS: usize = 2;
pub(super) const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;

pub use scheduler::{
    workout_summary_coach_reply_task_handler, SchedulerBackedWorkoutSummaryService,
};
pub(crate) use scheduler::{
    COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS, COACH_REPLY_LEASE_DURATION_SECONDS,
    COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS,
};

pub trait WorkoutSummaryUseCases: Send + Sync {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        self.get_summary_with_options(user_id, workout_id, WorkoutSummaryGetOptions::default())
    }

    fn create_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn list_summaries_with_options(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
        options: WorkoutSummaryListOptions,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>;

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        self.list_summaries_with_options(user_id, workout_ids, WorkoutSummaryListOptions::default())
    }

    fn get_summary_with_options(
        &self,
        user_id: &str,
        workout_id: &str,
        options: WorkoutSummaryGetOptions,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn mark_saved(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>>;

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn send_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>>;

    fn append_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>>;

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>>;
}

pub trait LatestCompletedActivityUseCases: Send + Sync {
    fn latest_completed_activity_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<String>, WorkoutSummaryError>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedCompletedWorkoutTarget {
    pub preferred_workout_id: String,
    pub equivalent_workout_ids: Vec<String>,
}

/// Date window for resolving completed-workout alias families without scanning full user history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedWorkoutAliasScope {
    pub oldest: String,
    pub newest: String,
}

impl CompletedWorkoutAliasScope {
    pub fn with_alias_margin_days(&self, margin_days: i64) -> Self {
        use chrono::{Duration, NaiveDate};

        let expanded_oldest = NaiveDate::parse_from_str(&self.oldest, "%Y-%m-%d")
            .ok()
            .and_then(|date| date.checked_sub_signed(Duration::days(margin_days)))
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| self.oldest.clone());
        let expanded_newest = NaiveDate::parse_from_str(&self.newest, "%Y-%m-%d")
            .ok()
            .and_then(|date| date.checked_add_signed(Duration::days(margin_days)))
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| self.newest.clone());

        Self {
            oldest: expanded_oldest,
            newest: expanded_newest,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkoutSummaryListOptions {
    pub alias_scope: Option<CompletedWorkoutAliasScope>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkoutSummaryGetOptions {
    pub alias_scope: Option<CompletedWorkoutAliasScope>,
}

pub trait CompletedWorkoutTargetUseCases: Send + Sync {
    fn is_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<bool, WorkoutSummaryError>>;

    fn resolve_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let is_completed = self.is_completed_workout_target(&user_id, &workout_id);
        Box::pin(async move {
            Ok(is_completed
                .await?
                .then_some(ResolvedCompletedWorkoutTarget {
                    preferred_workout_id: workout_id.clone(),
                    equivalent_workout_ids: vec![workout_id],
                }))
        })
    }

    fn resolve_completed_workout_target_in_scope(
        &self,
        user_id: &str,
        workout_id: &str,
        _alias_scope: &CompletedWorkoutAliasScope,
    ) -> BoxFuture<Result<Option<ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>> {
        self.resolve_completed_workout_target(user_id, workout_id)
    }

    fn resolve_completed_workout_targets_in_scope(
        &self,
        user_id: &str,
        workout_ids: &[String],
        alias_scope: &CompletedWorkoutAliasScope,
    ) -> BoxFuture<
        Result<
            std::collections::HashMap<String, ResolvedCompletedWorkoutTarget>,
            WorkoutSummaryError,
        >,
    >;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResolvedWorkoutSummaryTarget {
    requested_workout_id: String,
    preferred_workout_id: String,
    summary_workout_id: String,
    storage_workout_id: String,
    existing_summary: Option<WorkoutSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveWorkflowStatus {
    Generated,
    Processing,
    Skipped,
    Failed,
    Unchanged,
}

impl SaveWorkflowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Processing => "processing",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveWorkflowResult {
    pub recap_status: SaveWorkflowStatus,
    pub plan_status: SaveWorkflowStatus,
    pub messages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveSummaryResult {
    pub summary: WorkoutSummary,
    pub workflow: SaveWorkflowResult,
}

#[derive(Clone)]
pub struct WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    repository: Repo,
    reply_operations: Ops,
    clock: Time,
    ids: Ids,
    coach: Arc<dyn WorkoutCoach>,
    athlete_summary_service: Option<Arc<dyn AthleteSummaryUseCases>>,
    settings_service: Option<Arc<dyn UserSettingsUseCases>>,
    training_plan_service: Option<Arc<dyn TrainingPlanUseCases>>,
    latest_completed_activity_service: Option<Arc<dyn LatestCompletedActivityUseCases>>,
    completed_workout_target_service: Option<Arc<dyn CompletedWorkoutTargetUseCases>>,
    save_completion_port: Option<Arc<dyn SaveWorkflowCompletionPort>>,
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub fn new(repository: Repo, reply_operations: Ops, clock: Time, ids: Ids) -> Self {
        Self::with_coach(
            repository,
            reply_operations,
            clock,
            ids,
            Arc::new(super::MockWorkoutCoach),
        )
    }

    pub fn with_coach(
        repository: Repo,
        reply_operations: Ops,
        clock: Time,
        ids: Ids,
        coach: Arc<dyn WorkoutCoach>,
    ) -> Self {
        Self {
            repository,
            reply_operations,
            clock,
            ids,
            coach,
            athlete_summary_service: None,
            settings_service: None,
            training_plan_service: None,
            latest_completed_activity_service: None,
            completed_workout_target_service: None,
            save_completion_port: None,
        }
    }

    pub fn with_athlete_summary_service(
        mut self,
        athlete_summary_service: Arc<dyn AthleteSummaryUseCases>,
    ) -> Self {
        self.athlete_summary_service = Some(athlete_summary_service);
        self
    }

    pub fn with_settings_service(
        mut self,
        settings_service: Arc<dyn UserSettingsUseCases>,
    ) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    pub fn with_training_plan_service(
        mut self,
        training_plan_service: Arc<dyn TrainingPlanUseCases>,
    ) -> Self {
        self.training_plan_service = Some(training_plan_service);
        self
    }

    pub fn with_latest_completed_activity_service(
        mut self,
        latest_completed_activity_service: Arc<dyn LatestCompletedActivityUseCases>,
    ) -> Self {
        self.latest_completed_activity_service = Some(latest_completed_activity_service);
        self
    }

    pub fn with_completed_workout_target_service(
        mut self,
        completed_workout_target_service: Arc<dyn CompletedWorkoutTargetUseCases>,
    ) -> Self {
        self.completed_workout_target_service = Some(completed_workout_target_service);
        self
    }

    pub fn with_save_completion_port(
        mut self,
        save_completion_port: Arc<dyn SaveWorkflowCompletionPort>,
    ) -> Self {
        self.save_completion_port = Some(save_completion_port);
        self
    }
}
