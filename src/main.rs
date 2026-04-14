use std::{error::Error, future::Future, net::SocketAddr, sync::Arc, time::Duration};

use aiwattcoach::{
    adapters::{
        activity_file_identity::ActivityFileIdentityExtractor,
        google_oauth::{
            adapter::GoogleOAuthAdapter, client::GoogleOAuthClient,
            dev_client::DevGoogleOAuthClient,
        },
        intervals_icu::{
            adapter::IntervalsApiAdapter,
            client::IntervalsIcuClient,
            dev_client::DevIntervalsClient,
            dev_settings_adapter::DevIntervalsSettingsProvider,
            settings_adapter::{IntervalsSettingsAdapter, SettingsIntervalsProvider},
        },
        llm::{
            adapter::LlmAdapter, athlete_summary_generator::AthleteSummaryLlmGenerator,
            dev_adapter::DevLlmCoachAdapter, gemini::client::GeminiClient,
            openai::client::OpenAiClient, openrouter::client::OpenRouterClient,
            settings_adapter::SettingsLlmConfigProvider,
            training_plan_generator::TrainingPlanLlmGenerator,
            workout_summary_coach::LlmWorkoutCoach,
        },
        mongo::{
            activities::MongoActivityRepository,
            activity_upload_operations::MongoActivityUploadOperationRepository,
            athlete_summary::MongoAthleteSummaryRepository,
            athlete_summary_generation_operations::MongoAthleteSummaryGenerationOperationRepository,
            calendar_entry_view_calendar::MongoCalendarEntryViewCalendarSource,
            calendar_entry_views::MongoCalendarEntryViewRepository,
            client::{create_client, ensure_database_exists, verify_connection},
            coach_reply_operations::MongoCoachReplyOperationRepository,
            completed_workouts::MongoCompletedWorkoutRepository,
            external_observations::MongoExternalObservationRepository,
            external_sync_states::MongoExternalSyncStateRepository,
            llm_context_cache::MongoLlmContextCacheRepository,
            login_state::MongoLoginStateRepository,
            planned_workout_syncs::MongoPlannedWorkoutSyncRepository,
            planned_workouts::MongoPlannedWorkoutRepository,
            provider_poll_states::MongoProviderPollStateRepository,
            races::MongoRaceRepository,
            sessions::MongoSessionRepository,
            settings::MongoUserSettingsRepository,
            special_days::MongoSpecialDayRepository,
            training_plan_generation_operations::MongoTrainingPlanGenerationOperationRepository,
            training_plan_projections::MongoTrainingPlanProjectionRepository,
            training_plan_snapshots::MongoTrainingPlanSnapshotRepository,
            users::MongoUserRepository,
            workout_summary::MongoWorkoutSummaryRepository,
        },
        support::{SystemClock, UuidIdGenerator},
        workout_summary_latest_activity::LatestCompletedActivityAdapter,
    },
    build_app,
    config::{spawn_provider_polling_loop, ProviderPollingService, Settings},
    domain::athlete_summary::AthleteSummaryService,
    domain::calendar::CalendarService,
    domain::calendar_labels::CalendarLabelsService,
    domain::calendar_view::CalendarEntryViewRefreshService,
    domain::external_sync::{ExternalImportService, ProviderPollStateRepository},
    domain::identity::{
        validate_session_ttl_against_current_time, Clock, IdentityService, IdentityServiceConfig,
    },
    domain::intervals::IntervalsService,
    domain::races::RaceService,
    domain::settings::UserSettingsService,
    domain::training_context::DefaultTrainingContextBuilder,
    domain::training_plan::TrainingPlanGenerationService,
    domain::workout_summary::{WorkoutSummaryError, WorkoutSummaryService},
    telemetry::setup_telemetry,
    AppState,
};
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tracing::info;

#[derive(Clone)]
struct TrainingPlanWorkoutSummaryAdapter<Service> {
    workout_summary_service: Arc<Service>,
}

impl<Service> TrainingPlanWorkoutSummaryAdapter<Service> {
    fn new(workout_summary_service: Arc<Service>) -> Self {
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

async fn reconcile_intervals_poll_states(
    settings_repository: &MongoUserSettingsRepository,
    poll_states: &MongoProviderPollStateRepository,
    clock: &impl Clock,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let now_epoch_seconds = clock.now_epoch_seconds();
    let existing_intervals_user_ids = poll_states
        .list_user_ids_for_provider(aiwattcoach::domain::external_sync::ExternalProvider::Intervals)
        .await?;

    for user in settings_repository
        .list_intervals_poll_bootstrap_users(&existing_intervals_user_ids)
        .await?
    {
        for stream in [
            aiwattcoach::domain::external_sync::ProviderPollStream::Calendar,
            aiwattcoach::domain::external_sync::ProviderPollStream::CompletedWorkouts,
        ] {
            let existing = poll_states
                .find_by_provider_and_stream(
                    &user.user_id,
                    aiwattcoach::domain::external_sync::ExternalProvider::Intervals,
                    stream.clone(),
                )
                .await?;

            if !user.desired_active {
                if let Some(state) = existing {
                    poll_states
                        .upsert(aiwattcoach::domain::external_sync::ProviderPollState {
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
                    .upsert(aiwattcoach::domain::external_sync::ProviderPollState::new(
                        user.user_id.clone(),
                        aiwattcoach::domain::external_sync::ExternalProvider::Intervals,
                        stream,
                        now_epoch_seconds,
                    ))
                    .await?;
            }
        }
    }

    Ok(())
}

fn should_reset_poll_state(
    existing: Option<&aiwattcoach::domain::external_sync::ProviderPollState>,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let settings = Settings::from_env()?;
    let Settings {
        app_name,
        server,
        mongo,
        auth,
        dev_intervals_enabled,
        dev_llm_coach_enabled,
        client_log_ingestion_enabled,
        legacy_time_stream_cleanup_enabled,
    } = settings;
    let mut telemetry = setup_telemetry(&app_name)?;
    let address: SocketAddr = server.address().parse()?;
    let mongo_client = create_client(&mongo.uri).await?;
    ensure_database_exists(&mongo_client, &mongo.database).await?;
    verify_connection(&mongo_client, &mongo.database, Duration::from_secs(5)).await?;

    let mongo_database = mongo.database.clone();
    let user_repository = MongoUserRepository::new(mongo_client.clone(), &mongo_database);
    let session_repository = MongoSessionRepository::new(mongo_client.clone(), &mongo_database);
    let login_state_repository =
        MongoLoginStateRepository::new(mongo_client.clone(), &mongo_database);
    user_repository.ensure_indexes().await?;
    session_repository.ensure_indexes().await?;
    login_state_repository.ensure_indexes().await?;
    let google_oauth_client = if auth.dev.enabled {
        GoogleOAuthAdapter::Dev(DevGoogleOAuthClient::new(
            auth.dev.google_subject,
            auth.dev.email,
            auth.dev.display_name,
            auth.dev.avatar_url,
        ))
    } else {
        GoogleOAuthAdapter::Google(GoogleOAuthClient::new(
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .build()?,
            auth.google.client_id,
            auth.google.client_secret,
            auth.google.redirect_url,
        ))
    };
    validate_session_ttl_against_current_time(
        SystemClock.now_epoch_seconds(),
        auth.session.ttl_hours,
    )?;
    let identity_service = IdentityService::new(
        user_repository,
        session_repository,
        login_state_repository,
        google_oauth_client,
        SystemClock,
        UuidIdGenerator,
        IdentityServiceConfig::new(auth.admin_emails, auth.session.ttl_hours),
    );

    let settings_repository =
        MongoUserSettingsRepository::new(mongo_client.clone(), &mongo_database);
    settings_repository.ensure_indexes().await?;
    let llm_context_cache_repository =
        MongoLlmContextCacheRepository::new(mongo_client.clone(), &mongo_database);
    llm_context_cache_repository.ensure_indexes().await?;
    let llm_http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .build()?;
    let llm_adapter = if dev_llm_coach_enabled {
        Arc::new(LlmAdapter::Dev(DevLlmCoachAdapter))
    } else {
        Arc::new(LlmAdapter::live(
            OpenAiClient::new(llm_http_client.clone()),
            GeminiClient::new(llm_http_client.clone()),
            OpenRouterClient::new(llm_http_client),
        ))
    };
    let workout_summary_repository =
        MongoWorkoutSummaryRepository::new(mongo_client.clone(), &mongo_database);
    workout_summary_repository.ensure_indexes().await?;
    let athlete_summary_repository =
        MongoAthleteSummaryRepository::new(mongo_client.clone(), &mongo_database);
    athlete_summary_repository.ensure_indexes().await?;
    let athlete_summary_generation_operation_repository =
        MongoAthleteSummaryGenerationOperationRepository::new(
            mongo_client.clone(),
            &mongo_database,
        );
    athlete_summary_generation_operation_repository
        .ensure_indexes()
        .await?;
    let coach_reply_operation_repository =
        MongoCoachReplyOperationRepository::new(mongo_client.clone(), &mongo_database);
    coach_reply_operation_repository.ensure_indexes().await?;
    let training_plan_snapshot_repository =
        MongoTrainingPlanSnapshotRepository::new(mongo_client.clone(), &mongo_database);
    training_plan_snapshot_repository.ensure_indexes().await?;
    let training_plan_projection_repository =
        MongoTrainingPlanProjectionRepository::new(mongo_client.clone(), &mongo_database);
    training_plan_projection_repository.ensure_indexes().await?;
    let training_plan_generation_operation_repository =
        MongoTrainingPlanGenerationOperationRepository::new(mongo_client.clone(), &mongo_database);
    training_plan_generation_operation_repository
        .ensure_indexes()
        .await?;
    let planned_workout_sync_repository =
        MongoPlannedWorkoutSyncRepository::new(mongo_client.clone(), &mongo_database);
    planned_workout_sync_repository.ensure_indexes().await?;
    // These repositories are bootstrapped at startup so their durable collections
    // have indexes in place before background sync workflows start using them.
    let external_observation_repository =
        MongoExternalObservationRepository::new(mongo_client.clone(), &mongo_database);
    external_observation_repository.ensure_indexes().await?;
    let external_sync_state_repository =
        MongoExternalSyncStateRepository::new(mongo_client.clone(), &mongo_database);
    external_sync_state_repository.ensure_indexes().await?;
    let provider_poll_state_repository =
        MongoProviderPollStateRepository::new(mongo_client.clone(), &mongo_database);
    provider_poll_state_repository.ensure_indexes().await?;
    reconcile_intervals_poll_states(
        &settings_repository,
        &provider_poll_state_repository,
        &SystemClock,
    )
    .await?;
    let settings_service = Arc::new(
        UserSettingsService::new(settings_repository, SystemClock)
            .with_provider_poll_states(provider_poll_state_repository.clone())
            .with_llm_context_cache_repository(Arc::new(llm_context_cache_repository.clone())),
    );
    let llm_config_provider = Arc::new(SettingsLlmConfigProvider::new(settings_service.clone()));
    let race_repository = MongoRaceRepository::new(mongo_client.clone(), &mongo_database);
    race_repository.ensure_indexes().await?;
    let planned_workout_repository =
        MongoPlannedWorkoutRepository::new(mongo_client.clone(), &mongo_database);
    planned_workout_repository.ensure_indexes().await?;
    let completed_workout_repository =
        MongoCompletedWorkoutRepository::new(mongo_client.clone(), &mongo_database);
    completed_workout_repository.ensure_indexes().await?;
    let special_day_repository =
        MongoSpecialDayRepository::new(mongo_client.clone(), &mongo_database);
    special_day_repository.ensure_indexes().await?;
    let calendar_entry_view_repository =
        MongoCalendarEntryViewRepository::new(mongo_client.clone(), &mongo_database);
    calendar_entry_view_repository.ensure_indexes().await?;
    let activity_repository = MongoActivityRepository::new(mongo_client.clone(), &mongo_database);
    activity_repository.ensure_indexes().await?;
    if legacy_time_stream_cleanup_enabled {
        let cleaned_activity_documents = activity_repository.cleanup_legacy_time_streams().await?;
        if cleaned_activity_documents > 0 {
            info!(
                cleaned_activity_documents,
                "Removed legacy time streams from stored activities"
            );
        }
    }
    let upload_operation_repository =
        MongoActivityUploadOperationRepository::new(mongo_client.clone(), &mongo_database);
    upload_operation_repository.ensure_indexes().await?;
    let calendar_entry_view_refresh_service = CalendarEntryViewRefreshService::new(
        calendar_entry_view_repository.clone(),
        planned_workout_repository.clone(),
        planned_workout_sync_repository.clone(),
        completed_workout_repository.clone(),
        race_repository.clone(),
        special_day_repository.clone(),
        external_sync_state_repository.clone(),
    );
    let intervals_api_client = if dev_intervals_enabled {
        IntervalsApiAdapter::Dev(DevIntervalsClient)
    } else {
        IntervalsApiAdapter::Live(IntervalsIcuClient::with_timeouts(10, 30)?)
    };
    let intervals_settings_provider = if dev_intervals_enabled {
        IntervalsSettingsAdapter::Dev(DevIntervalsSettingsProvider)
    } else {
        IntervalsSettingsAdapter::Live(SettingsIntervalsProvider::new(settings_service.clone()))
    };
    let external_import_service = ExternalImportService::new(
        planned_workout_repository.clone(),
        completed_workout_repository.clone(),
        race_repository.clone(),
        special_day_repository.clone(),
        external_observation_repository.clone(),
        external_sync_state_repository.clone(),
        SystemClock,
    )
    .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone());
    let provider_polling_service = ProviderPollingService::new(
        intervals_api_client.clone(),
        intervals_settings_provider.clone(),
        provider_poll_state_repository.clone(),
        external_import_service,
        SystemClock,
        UuidIdGenerator,
    )
    .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone());
    let activity_identity_extractor = ActivityFileIdentityExtractor;
    let intervals_service = Arc::new(
        IntervalsService::new(
            intervals_api_client,
            intervals_settings_provider,
            activity_repository.clone(),
            upload_operation_repository,
            activity_identity_extractor,
        )
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );

    let training_context_builder = Arc::new(
        DefaultTrainingContextBuilder::new(
            settings_service.clone(),
            intervals_service.clone(),
            Arc::new(workout_summary_repository.clone()),
            SystemClock,
        )
        .with_race_repository(Arc::new(race_repository.clone()))
        .with_training_plan_projection_repository(Arc::new(
            training_plan_projection_repository.clone(),
        )),
    );
    let athlete_summary_service = Arc::new(AthleteSummaryService::new(
        athlete_summary_repository,
        athlete_summary_generation_operation_repository,
        AthleteSummaryLlmGenerator::new(
            llm_adapter.clone(),
            llm_config_provider.clone(),
            training_context_builder.clone(),
        ),
        SystemClock,
    ));

    let workout_summary_service = Arc::new(
        WorkoutSummaryService::with_coach(
            workout_summary_repository.clone(),
            coach_reply_operation_repository.clone(),
            SystemClock,
            UuidIdGenerator,
            Arc::new(
                LlmWorkoutCoach::new(
                    llm_adapter.clone(),
                    llm_config_provider.clone(),
                    training_context_builder.clone(),
                    SystemClock,
                )
                .with_context_cache_repository(Arc::new(llm_context_cache_repository)),
            ),
        )
        .with_athlete_summary_service(athlete_summary_service.clone())
        .with_settings_service(settings_service.clone())
        .with_latest_completed_activity_service(Arc::new(
            LatestCompletedActivityAdapter::new(activity_repository.clone()),
        )),
    );
    let training_plan_service = Arc::new(
        TrainingPlanGenerationService::new(
            training_plan_snapshot_repository,
            training_plan_projection_repository.clone(),
            training_plan_generation_operation_repository,
            TrainingPlanLlmGenerator::new(
                llm_adapter.clone(),
                llm_config_provider.clone(),
                training_context_builder.clone(),
                SystemClock,
            ),
            TrainingPlanWorkoutSummaryAdapter::new(workout_summary_service.clone()),
            SystemClock,
        )
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );
    let race_service = Arc::new(
        RaceService::new(
            race_repository.clone(),
            (*intervals_service).clone(),
            external_sync_state_repository.clone(),
            SystemClock,
            UuidIdGenerator,
        )
        .with_provider_poll_states(provider_poll_state_repository.clone())
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );
    let race_calendar_source =
        MongoCalendarEntryViewCalendarSource::new(mongo_client.clone(), &mongo_database);
    let calendar_labels_service =
        Arc::new(CalendarLabelsService::new(race_calendar_source.clone()));
    let calendar_service = Arc::new(
        CalendarService::new(
            (*intervals_service).clone(),
            training_plan_projection_repository.clone(),
            planned_workout_sync_repository,
            race_calendar_source,
            SystemClock,
        )
        .with_provider_poll_states(provider_poll_state_repository)
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );
    let workout_summary_service = Arc::new(
        (*workout_summary_service)
            .clone()
            .with_training_plan_service(training_plan_service),
    );

    let intervals_connection_tester = if dev_intervals_enabled {
        IntervalsApiAdapter::Dev(DevIntervalsClient)
    } else {
        IntervalsApiAdapter::Live(IntervalsIcuClient::with_timeouts(5, 15)?)
    };

    let app = build_app(
        AppState::new(app_name, mongo_database, mongo_client)
            .with_client_log_ingestion(client_log_ingestion_enabled)
            .with_identity_service(
                Arc::new(identity_service),
                auth.session.cookie_name,
                auth.session.same_site,
                auth.session.secure,
                auth.session.ttl_hours,
            )
            .with_settings_service(settings_service)
            .with_calendar_service(calendar_service)
            .with_calendar_labels_service(calendar_labels_service)
            .with_athlete_summary_service(athlete_summary_service)
            .with_llm_services(llm_adapter, llm_config_provider)
            .with_workout_summary_service(workout_summary_service)
            .with_intervals_service(intervals_service)
            .with_race_service(race_service)
            .with_intervals_connection_tester(Arc::new(intervals_connection_tester)),
    );
    let listener = TcpListener::bind(address).await?;
    spawn_provider_polling_loop(provider_polling_service);

    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
    let telemetry_shutdown_result = telemetry.shutdown();

    finish_server_shutdown(serve_result, telemetry_shutdown_result)
}

fn finish_server_shutdown(
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

async fn shutdown_signal() {
    let shutdown = Arc::new(Notify::new());
    let ctrl_c = wait_for_ctrl_c(tokio::signal::ctrl_c(), shutdown.clone());

    #[cfg(unix)]
    let terminate = wait_for_sigterm(
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()),
        shutdown,
    );

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn wait_for_ctrl_c<F>(ctrl_c: F, shutdown: Arc<Notify>)
where
    F: Future<Output = std::io::Result<()>>,
{
    match ctrl_c.await {
        Ok(()) => shutdown.notify_waiters(),
        Err(error) => {
            tracing::error!(%error, "Failed to listen for Ctrl+C");
            shutdown.notified().await;
        }
    }
}

#[cfg(unix)]
async fn wait_for_sigterm(
    signal: std::io::Result<tokio::signal::unix::Signal>,
    shutdown: Arc<Notify>,
) {
    match signal {
        Ok(mut signal) => {
            signal.recv().await;
            shutdown.notify_waiters();
        }
        Err(error) => {
            tracing::error!(%error, "Failed to listen for SIGTERM");
            shutdown.notified().await;
        }
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
