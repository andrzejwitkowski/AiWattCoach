use std::{collections::BTreeSet, error::Error, future::Future, sync::Arc};

use crate::{
    adapters::mongo::{
        provider_poll_states::MongoProviderPollStateRepository,
        settings::MongoUserSettingsRepository,
    },
    domain::{
        external_sync::{
            ExternalProvider, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
        },
        identity::Clock,
        wahoo::WahooUseCases,
        workout_summary::WorkoutSummaryError,
    },
};
use tokio::sync::Notify;
use tracing::warn;

#[derive(Clone)]
pub struct TrainingPlanWorkoutSummaryAdapter<Service> {
    workout_summary_service: Arc<Service>,
}

impl<Service> TrainingPlanWorkoutSummaryAdapter<Service> {
    pub fn new(workout_summary_service: Arc<Service>) -> Self {
        Self {
            workout_summary_service,
        }
    }
}

impl<Service> crate::domain::training_plan::TrainingPlanWorkoutSummaryPort
    for TrainingPlanWorkoutSummaryAdapter<Service>
where
    Service: crate::domain::workout_summary::WorkoutSummaryUseCases + Send + Sync + 'static,
{
    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: crate::domain::workout_summary::WorkoutRecap,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<(), crate::domain::training_plan::TrainingPlanError>,
    > {
        let workout_summary_service = self.workout_summary_service.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            workout_summary_service
                .persist_workout_recap(&user_id, &workout_id, recap)
                .await
                .map(|_| ())
                .map_err(map_workout_summary_error)
        })
    }

    fn get_planning_context(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<
            Option<crate::domain::training_plan::TrainingPlanPlanningContext>,
            crate::domain::training_plan::TrainingPlanError,
        >,
    > {
        let workout_summary_service = self.workout_summary_service.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            workout_summary_service
                .get_summary(&user_id, &workout_id)
                .await
                .map(crate::domain::training_plan::map_workout_summary_to_planning_context)
                .map_err(map_workout_summary_error)
        })
    }
}

fn map_workout_summary_error(
    error: WorkoutSummaryError,
) -> crate::domain::training_plan::TrainingPlanError {
    match error {
        WorkoutSummaryError::Validation(message) => {
            crate::domain::training_plan::TrainingPlanError::Validation(message)
        }
        WorkoutSummaryError::Locked => crate::domain::training_plan::TrainingPlanError::Validation(
            "workout summary is saved and cannot be edited".to_string(),
        ),
        WorkoutSummaryError::NotFound => {
            crate::domain::training_plan::TrainingPlanError::Validation(
                "workout summary not found".to_string(),
            )
        }
        WorkoutSummaryError::AlreadyExists => {
            crate::domain::training_plan::TrainingPlanError::Validation(
                "workout summary already exists".to_string(),
            )
        }
        WorkoutSummaryError::ReplyAlreadyPending => {
            crate::domain::training_plan::TrainingPlanError::Unavailable(
                "coach reply generation is already pending for this message".to_string(),
            )
        }
        WorkoutSummaryError::Llm(error) => {
            crate::domain::training_plan::TrainingPlanError::Unavailable(error.to_string())
        }
        WorkoutSummaryError::Repository(message) => {
            crate::domain::training_plan::TrainingPlanError::Repository(message)
        }
    }
}

pub async fn reconcile_intervals_poll_states(
    settings_repository: &MongoUserSettingsRepository,
    poll_states: &MongoProviderPollStateRepository,
    clock: &impl Clock,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let now_epoch_seconds = clock.now_epoch_seconds();
    let existing_intervals_user_ids = poll_states
        .list_user_ids_for_provider(ExternalProvider::Intervals)
        .await?;

    for user in settings_repository
        .list_intervals_poll_bootstrap_users(&existing_intervals_user_ids)
        .await?
    {
        if let Some(calendar_state) = poll_states
            .find_by_provider_and_stream(
                &user.user_id,
                ExternalProvider::Intervals,
                ProviderPollStream::Calendar,
            )
            .await?
        {
            poll_states
                .upsert(ProviderPollState {
                    next_due_at_epoch_seconds: i64::MAX,
                    cursor: None,
                    backoff_until_epoch_seconds: None,
                    last_error: None,
                    ..calendar_state
                })
                .await?;
        }

        let existing = poll_states
            .find_by_provider_and_stream(
                &user.user_id,
                ExternalProvider::Intervals,
                ProviderPollStream::CompletedWorkouts,
            )
            .await?;

        if !user.desired_active {
            if let Some(state) = existing {
                poll_states
                    .upsert(ProviderPollState {
                        next_due_at_epoch_seconds: i64::MAX,
                        cursor: None,
                        backoff_until_epoch_seconds: None,
                        last_error: None,
                        ..state
                    })
                    .await?;
            }
            continue;
        }

        if should_reset_poll_state(existing.as_ref(), user.intervals_updated_at_epoch_seconds) {
            poll_states
                .upsert(ProviderPollState::new(
                    user.user_id.clone(),
                    ExternalProvider::Intervals,
                    ProviderPollStream::CompletedWorkouts,
                    now_epoch_seconds,
                ))
                .await?;
        }
    }

    Ok(())
}

pub async fn park_wahoo_poll_states(
    poll_states: &MongoProviderPollStateRepository,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let existing_wahoo_user_ids = poll_states
        .list_user_ids_for_provider(ExternalProvider::Wahoo)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();

    for user_id in existing_wahoo_user_ids {
        let Some(state) = poll_states
            .find_by_provider_and_stream(
                &user_id,
                ExternalProvider::Wahoo,
                ProviderPollStream::CompletedWorkouts,
            )
            .await?
        else {
            continue;
        };

        poll_states
            .upsert(ProviderPollState {
                next_due_at_epoch_seconds: i64::MAX,
                cursor: None,
                backoff_until_epoch_seconds: None,
                last_error: None,
                ..state
            })
            .await?;
    }

    Ok(())
}

pub async fn reconcile_wahoo_user_ids(
    settings_repository: &MongoUserSettingsRepository,
    wahoo_service: &dyn WahooUseCases,
    clock: &impl Clock,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let now_epoch_seconds = clock.now_epoch_seconds();
    let users = settings_repository
        .list_wahoo_user_id_backfill_candidates()
        .await?;

    for user in users {
        let wahoo_user = match wahoo_service.get_authenticated_user(&user.user_id).await {
            Ok(wahoo_user) => wahoo_user,
            Err(error) => {
                warn!(
                    user_id = %user.user_id,
                    error = %error,
                    "Failed to backfill Wahoo user id during startup reconcile"
                );
                continue;
            }
        };

        if let Err(error) = settings_repository
            .backfill_wahoo_user_id(&user.user_id, wahoo_user.id, now_epoch_seconds)
            .await
        {
            warn!(
                user_id = %user.user_id,
                wahoo_user_id = wahoo_user.id,
                error = %error,
                "Failed to persist Wahoo user id during startup reconcile"
            );
        }
    }

    Ok(())
}

pub fn should_reset_poll_state(
    existing: Option<&ProviderPollState>,
    intervals_updated_at_epoch_seconds: Option<i64>,
) -> bool {
    match existing {
        None => true,
        Some(state) => {
            let Some(intervals_updated_at_epoch_seconds) = intervals_updated_at_epoch_seconds
            else {
                return false;
            };
            let poll_touched_at_epoch_seconds = state
                .last_successful_at_epoch_seconds
                .into_iter()
                .chain(state.last_attempted_at_epoch_seconds)
                .max()
                .unwrap_or(i64::MIN);

            intervals_updated_at_epoch_seconds > poll_touched_at_epoch_seconds
                && (state.next_due_at_epoch_seconds == i64::MAX
                    || state.cursor.is_some()
                    || state.backoff_until_epoch_seconds.is_some()
                    || state.last_error.is_some())
        }
    }
}

pub fn finish_server_shutdown(
    serve_result: std::io::Result<()>,
    telemetry_shutdown_result: Result<(), Box<dyn Error + Send + Sync>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match (serve_result, telemetry_shutdown_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(serve_error), Ok(())) => Err(Box::new(serve_error)),
        (Ok(()), Err(telemetry_error)) => Err(telemetry_error),
        (Err(serve_error), Err(telemetry_error)) => Err(Box::new(std::io::Error::other(format!(
            "server failed: {serve_error}; telemetry shutdown failed: {telemetry_error}"
        )))),
    }
}

pub async fn shutdown_signal() {
    let shutdown = Arc::new(Notify::new());
    let ctrl_c = wait_for_ctrl_c(tokio::signal::ctrl_c(), shutdown.clone());

    #[cfg(unix)]
    let terminate = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        Ok(signal) => Some(wait_for_sigterm(Ok(signal), shutdown)),
        Err(error) => {
            tracing::error!(%error, "Failed to register SIGTERM handler");
            None
        }
    };

    #[cfg(not(unix))]
    let _terminate = ();

    #[cfg(unix)]
    {
        if let Some(terminate) = terminate {
            tokio::select! {
                _ = ctrl_c => {},
                _ = terminate => {},
            }
        } else {
            ctrl_c.await;
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

pub async fn wait_for_ctrl_c<F>(ctrl_c: F, shutdown: Arc<Notify>)
where
    F: Future<Output = std::io::Result<()>>,
{
    match ctrl_c.await {
        Ok(()) => shutdown.notify_waiters(),
        Err(error) => tracing::error!(%error, "Failed to listen for Ctrl+C"),
    }
}

#[cfg(unix)]
pub async fn wait_for_sigterm(
    signal: std::io::Result<tokio::signal::unix::Signal>,
    shutdown: Arc<Notify>,
) {
    match signal {
        Ok(mut signal) => {
            signal.recv().await;
            shutdown.notify_waiters();
        }
        Err(error) => tracing::error!(%error, "Failed to listen for SIGTERM"),
    }
}

#[cfg(test)]
mod tests {
    use super::TrainingPlanWorkoutSummaryAdapter;
    use crate::domain::{
        llm::LlmChatMessage,
        training_plan::TrainingPlanWorkoutSummaryPort,
        workout_summary::{
            BoxFuture, CoachReply, ConversationMessage, MessageRole, PersistedUserMessage,
            SaveSummaryResult, SendMessageResult, WorkoutRecap, WorkoutSummary,
            WorkoutSummaryError, WorkoutSummaryUseCases,
        },
    };

    #[derive(Clone)]
    struct NotFoundWorkoutSummaryService;

    impl WorkoutSummaryUseCases for NotFoundWorkoutSummaryService {
        fn get_summary(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            Box::pin(async { Err(WorkoutSummaryError::NotFound) })
        }

        fn create_summary(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn list_summaries(
            &self,
            _user_id: &str,
            _workout_ids: Vec<String>,
        ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
            unreachable!()
        }

        fn list_summaries_with_options(
            &self,
            user_id: &str,
            workout_ids: Vec<String>,
            _options: crate::domain::workout_summary::WorkoutSummaryListOptions,
        ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
            self.list_summaries(user_id, workout_ids)
        }

        fn get_summary_with_options(
            &self,
            user_id: &str,
            workout_id: &str,
            _options: crate::domain::workout_summary::WorkoutSummaryGetOptions,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            self.get_summary(user_id, workout_id)
        }

        fn update_rpe(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _rpe: u8,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn mark_saved(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>> {
            unreachable!()
        }

        fn reopen_summary(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn persist_workout_recap(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _recap: WorkoutRecap,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn send_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _content: String,
        ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
            unreachable!()
        }

        fn append_user_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _content: String,
        ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
            unreachable!()
        }

        fn generate_coach_reply(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _user_message_id: String,
        ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn planning_context_not_found_maps_to_validation_error() {
        let adapter = TrainingPlanWorkoutSummaryAdapter::new(std::sync::Arc::new(
            NotFoundWorkoutSummaryService,
        ));

        let error = adapter
            .get_planning_context("user-1", "workout-1")
            .await
            .unwrap_err();

        assert_eq!(
            error,
            crate::domain::training_plan::TrainingPlanError::Validation(
                "workout summary not found".to_string()
            )
        );
    }

    #[derive(Clone)]
    struct StaticWorkoutSummaryService {
        summary: WorkoutSummary,
    }

    impl WorkoutSummaryUseCases for StaticWorkoutSummaryService {
        fn get_summary(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            let summary = self.summary.clone();
            Box::pin(async move { Ok(summary) })
        }

        fn create_summary(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn list_summaries(
            &self,
            _user_id: &str,
            _workout_ids: Vec<String>,
        ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
            unreachable!()
        }

        fn list_summaries_with_options(
            &self,
            user_id: &str,
            workout_ids: Vec<String>,
            _options: crate::domain::workout_summary::WorkoutSummaryListOptions,
        ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
            self.list_summaries(user_id, workout_ids)
        }

        fn get_summary_with_options(
            &self,
            user_id: &str,
            workout_id: &str,
            _options: crate::domain::workout_summary::WorkoutSummaryGetOptions,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            self.get_summary(user_id, workout_id)
        }

        fn update_rpe(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _rpe: u8,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn mark_saved(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>> {
            unreachable!()
        }

        fn reopen_summary(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn persist_workout_recap(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _recap: WorkoutRecap,
        ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
            unreachable!()
        }

        fn send_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _content: String,
        ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
            unreachable!()
        }

        fn append_user_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _content: String,
        ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
            unreachable!()
        }

        fn generate_coach_reply(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _user_message_id: String,
        ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn planning_context_ignores_public_tool_messages() {
        let adapter = TrainingPlanWorkoutSummaryAdapter::new(std::sync::Arc::new(
            StaticWorkoutSummaryService {
                summary: WorkoutSummary {
                    id: "summary-1".to_string(),
                    user_id: "user-1".to_string(),
                    workout_id: "workout-1".to_string(),
                    rpe: Some(6),
                    messages: vec![
                        ConversationMessage {
                            id: "user-1".to_string(),
                            role: MessageRole::User,
                            content: "How did I do?".to_string(),
                            tool_call: None,
                            questions: Vec::new(),
                            created_at_epoch_seconds: 1,
                        },
                        ConversationMessage {
                            id: "tool-1".to_string(),
                            role: MessageRole::Tool,
                            content: "Tool call: lookupWorkout".to_string(),
                            tool_call: None,
                            questions: Vec::new(),
                            created_at_epoch_seconds: 2,
                        },
                        ConversationMessage {
                            id: "coach-1".to_string(),
                            role: MessageRole::Coach,
                            content: "You faded late.".to_string(),
                            tool_call: None,
                            questions: Vec::new(),
                            created_at_epoch_seconds: 3,
                        },
                    ],
                    provider_transcript: vec![LlmChatMessage::assistant(
                        crate::domain::workout_summary::coach_reply_json("You faded late."),
                    )],
                    saved_at_epoch_seconds: None,
                    workout_recap_text: None,
                    workout_recap_provider: None,
                    workout_recap_model: None,
                    workout_recap_generated_at_epoch_seconds: None,
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 3,
                },
            },
        ));

        let context = adapter
            .get_planning_context("user-1", "workout-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(context.messages.len(), 2);
        assert_eq!(context.messages[0].content, "How did I do?");
        assert_eq!(context.messages[1].content, "You faded late.");
    }
}
