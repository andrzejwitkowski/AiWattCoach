use std::{error::Error, future::Future, sync::Arc};

use aiwattcoach::{
    adapters::mongo::{
        provider_poll_states::MongoProviderPollStateRepository,
        settings::MongoUserSettingsRepository,
    },
    domain::{
        external_sync::{
            ExternalProvider, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
        },
        identity::Clock,
        workout_summary::WorkoutSummaryError,
    },
};
use tokio::sync::Notify;

#[derive(Clone)]
pub(crate) struct TrainingPlanWorkoutSummaryAdapter<Service> {
    workout_summary_service: Arc<Service>,
}

impl<Service> TrainingPlanWorkoutSummaryAdapter<Service> {
    pub(crate) fn new(workout_summary_service: Arc<Service>) -> Self {
        Self {
            workout_summary_service,
        }
    }
}

impl<Service> aiwattcoach::domain::training_plan::TrainingPlanWorkoutSummaryPort
    for TrainingPlanWorkoutSummaryAdapter<Service>
where
    Service: aiwattcoach::domain::workout_summary::WorkoutSummaryUseCases + Send + Sync + 'static,
{
    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: aiwattcoach::domain::workout_summary::WorkoutRecap,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<(), aiwattcoach::domain::training_plan::TrainingPlanError>,
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
}

fn map_workout_summary_error(
    error: WorkoutSummaryError,
) -> aiwattcoach::domain::training_plan::TrainingPlanError {
    match error {
        WorkoutSummaryError::Validation(message) => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Validation(message)
        }
        WorkoutSummaryError::Locked => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Validation(
                "workout summary is saved and cannot be edited".to_string(),
            )
        }
        WorkoutSummaryError::NotFound => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Validation(
                "workout summary not found".to_string(),
            )
        }
        WorkoutSummaryError::AlreadyExists => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Validation(
                "workout summary already exists".to_string(),
            )
        }
        WorkoutSummaryError::ReplyAlreadyPending => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
                "coach reply generation is already pending for this message".to_string(),
            )
        }
        WorkoutSummaryError::Llm(error) => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(error.to_string())
        }
        WorkoutSummaryError::Repository(message) => {
            aiwattcoach::domain::training_plan::TrainingPlanError::Repository(message)
        }
    }
}

pub(crate) async fn reconcile_intervals_poll_states(
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
        for stream in [
            ProviderPollStream::Calendar,
            ProviderPollStream::CompletedWorkouts,
        ] {
            let existing = poll_states
                .find_by_provider_and_stream(
                    &user.user_id,
                    ExternalProvider::Intervals,
                    stream.clone(),
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
                        stream,
                        now_epoch_seconds,
                    ))
                    .await?;
            }
        }
    }

    Ok(())
}

pub(crate) fn should_reset_poll_state(
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

pub(crate) fn finish_server_shutdown(
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

pub(crate) async fn shutdown_signal() {
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

pub(crate) async fn wait_for_ctrl_c<F>(ctrl_c: F, shutdown: Arc<Notify>)
where
    F: Future<Output = std::io::Result<()>>,
{
    match ctrl_c.await {
        Ok(()) => shutdown.notify_waiters(),
        Err(error) => tracing::error!(%error, "Failed to listen for Ctrl+C"),
    }
}

#[cfg(unix)]
pub(crate) async fn wait_for_sigterm(
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
