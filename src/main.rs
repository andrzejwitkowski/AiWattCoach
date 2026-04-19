use std::{error::Error, net::SocketAddr, sync::Arc, time::Duration};

use aiwattcoach::{
    adapters::{
        activity_file_identity::ActivityFileIdentityExtractor,
        google_oauth::{
            adapter::GoogleOAuthAdapter, client::GoogleOAuthClient,
            dev_client::DevGoogleOAuthClient,
        },
        intervals_icu::{
            adapter::IntervalsApiAdapter,
            backfill::IntervalsCompletedWorkoutBackfillService,
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
            ftp_history::MongoFtpHistoryRepository,
            llm_context_cache::MongoLlmContextCacheRepository,
            login_state::MongoLoginStateRepository,
            planned_completed_links::MongoPlannedCompletedWorkoutLinkRepository,
            planned_workout_syncs::MongoPlannedWorkoutSyncRepository,
            planned_workout_tokens::MongoPlannedWorkoutTokenRepository,
            planned_workouts::MongoPlannedWorkoutRepository,
            provider_poll_states::MongoProviderPollStateRepository,
            races::MongoRaceRepository,
            sessions::MongoSessionRepository,
            settings::MongoUserSettingsRepository,
            special_days::MongoSpecialDayRepository,
            training_load_daily_snapshots::MongoTrainingLoadDailySnapshotRepository,
            training_plan_generation_operations::MongoTrainingPlanGenerationOperationRepository,
            training_plan_projections::MongoTrainingPlanProjectionRepository,
            training_plan_snapshots::MongoTrainingPlanSnapshotRepository,
            users::MongoUserRepository,
            whitelist::MongoWhitelistRepository,
            workout_summary::MongoWorkoutSummaryRepository,
        },
        support::{SystemClock, UuidIdGenerator},
        workout_summary_completed_target::CompletedWorkoutTargetAdapter,
        workout_summary_latest_activity::LatestCompletedActivityAdapter,
    },
    build_app,
    config::{spawn_provider_polling_loop, ProviderPollingService, Settings},
    domain::athlete_summary::AthleteSummaryService,
    domain::calendar::CalendarService,
    domain::calendar_labels::CalendarLabelsService,
    domain::calendar_view::CalendarEntryViewRefreshService,
    domain::completed_workouts::CompletedWorkoutReadService,
    domain::external_sync::ExternalImportService,
    domain::identity::{
        validate_session_ttl_against_current_time, Clock, IdentityService, IdentityServiceConfig,
        IdentityServiceDependencies,
    },
    domain::intervals::IntervalsService,
    domain::races::RaceService,
    domain::settings::UserSettingsService,
    domain::training_context::DefaultTrainingContextBuilder,
    domain::training_load::{TrainingLoadDashboardReadService, TrainingLoadRecomputeService},
    domain::training_plan::TrainingPlanGenerationService,
    domain::workout_summary::WorkoutSummaryService,
    telemetry::setup_telemetry,
    AppState,
};
use tokio::net::TcpListener;
use tracing::info;

mod main_support;

use main_support::{
    finish_server_shutdown, reconcile_intervals_poll_states, shutdown_signal,
    TrainingPlanWorkoutSummaryAdapter,
};

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
        trust_proxy_headers,
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
    let whitelist_repository = MongoWhitelistRepository::new(mongo_client.clone(), &mongo_database);
    user_repository.ensure_indexes().await?;
    session_repository.ensure_indexes().await?;
    login_state_repository.ensure_indexes().await?;
    whitelist_repository.ensure_indexes().await?;
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
        IdentityServiceDependencies {
            users: user_repository,
            sessions: session_repository,
            login_states: login_state_repository,
            whitelist: whitelist_repository,
            google_oauth: google_oauth_client,
            clock: SystemClock,
            ids: UuidIdGenerator,
        },
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
    let ftp_history_repository =
        MongoFtpHistoryRepository::new(mongo_client.clone(), &mongo_database);
    ftp_history_repository.ensure_indexes().await?;
    let training_load_daily_snapshot_repository =
        MongoTrainingLoadDailySnapshotRepository::new(mongo_client.clone(), &mongo_database);
    training_load_daily_snapshot_repository
        .ensure_indexes()
        .await?;
    reconcile_intervals_poll_states(
        &settings_repository,
        &provider_poll_state_repository,
        &SystemClock,
    )
    .await?;
    let race_repository = MongoRaceRepository::new(mongo_client.clone(), &mongo_database);
    race_repository.ensure_indexes().await?;
    let planned_workout_repository =
        MongoPlannedWorkoutRepository::new(mongo_client.clone(), &mongo_database);
    planned_workout_repository.ensure_indexes().await?;
    let planned_workout_token_repository =
        MongoPlannedWorkoutTokenRepository::new(mongo_client.clone(), &mongo_database);
    planned_workout_token_repository.ensure_indexes().await?;
    let planned_completed_link_repository =
        MongoPlannedCompletedWorkoutLinkRepository::new(mongo_client.clone(), &mongo_database);
    planned_completed_link_repository.ensure_indexes().await?;
    let completed_workout_repository =
        MongoCompletedWorkoutRepository::new(mongo_client.clone(), &mongo_database);
    completed_workout_repository.ensure_indexes().await?;
    let training_load_recompute_service = Arc::new(TrainingLoadRecomputeService::new(
        completed_workout_repository.clone(),
        ftp_history_repository.clone(),
        training_load_daily_snapshot_repository.clone(),
        settings_repository.clone(),
    ));
    let training_load_dashboard_service = Arc::new(TrainingLoadDashboardReadService::new(
        training_load_daily_snapshot_repository.clone(),
    ));
    let settings_service = Arc::new(
        UserSettingsService::new(settings_repository, SystemClock)
            .with_provider_poll_states(provider_poll_state_repository.clone())
            .with_llm_context_cache_repository(Arc::new(llm_context_cache_repository.clone()))
            .with_ftp_history_repository(ftp_history_repository.clone())
            .with_training_load_recompute_service(training_load_recompute_service.clone()),
    );
    let llm_config_provider = Arc::new(SettingsLlmConfigProvider::new(settings_service.clone()));
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
    )
    .with_planned_completed_links(planned_completed_link_repository.clone());
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
        planned_workout_token_repository.clone(),
        planned_completed_link_repository.clone(),
        external_observation_repository.clone(),
        external_sync_state_repository.clone(),
        SystemClock,
    )
    .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone());
    let provider_polling_service = ProviderPollingService::new(
        intervals_api_client.clone(),
        intervals_settings_provider.clone(),
        provider_poll_state_repository.clone(),
        external_import_service.clone(),
        SystemClock,
        UuidIdGenerator,
    )
    .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone())
    .with_training_load_recompute_service(training_load_recompute_service.clone());
    let activity_identity_extractor = ActivityFileIdentityExtractor;
    let intervals_service = Arc::new(
        IntervalsService::new(
            intervals_api_client.clone(),
            intervals_settings_provider.clone(),
            activity_repository.clone(),
            upload_operation_repository,
            activity_identity_extractor,
        )
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );

    let training_context_builder = Arc::new(
        DefaultTrainingContextBuilder::new(
            settings_service.clone(),
            Arc::new(workout_summary_repository.clone()),
            SystemClock,
        )
        .with_completed_workout_repository(completed_workout_repository.clone())
        .with_planned_workout_repository(planned_workout_repository.clone())
        .with_special_day_repository(special_day_repository.clone())
        .with_ftp_history_repository(ftp_history_repository.clone())
        .with_training_load_daily_snapshot_repository(
            training_load_daily_snapshot_repository.clone(),
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
        .with_completed_workout_target_service(Arc::new(CompletedWorkoutTargetAdapter::new(
            completed_workout_repository.clone(),
        )))
        .with_latest_completed_activity_service(Arc::new(
            LatestCompletedActivityAdapter::new(completed_workout_repository.clone()),
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
    let completed_workout_service = Arc::new(CompletedWorkoutReadService::new(
        completed_workout_repository.clone(),
    ));
    let completed_workout_admin_service = Arc::new(IntervalsCompletedWorkoutBackfillService::new(
        completed_workout_repository.clone(),
        intervals_settings_provider.clone(),
        intervals_api_client.clone(),
        external_import_service.clone(),
    ));
    let calendar_service = Arc::new(
        CalendarService::new(
            (*intervals_service).clone(),
            calendar_entry_view_repository.clone(),
            training_plan_projection_repository.clone(),
            planned_workout_sync_repository,
            SystemClock,
        )
        .with_planned_workout_tokens(planned_workout_token_repository)
        .with_provider_poll_states(provider_poll_state_repository)
        .with_completed_workouts(completed_workout_repository.clone())
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
            .with_trust_proxy_headers(trust_proxy_headers)
            .with_identity_service(
                Arc::new(identity_service),
                auth.session.cookie_name,
                auth.session.same_site,
                auth.session.secure,
                auth.session.ttl_hours,
            )
            .with_settings_service(settings_service)
            .with_training_load_dashboard_service(training_load_dashboard_service)
            .with_calendar_service(calendar_service)
            .with_calendar_labels_service(calendar_labels_service)
            .with_completed_workout_service(completed_workout_service)
            .with_completed_workout_admin_service(completed_workout_admin_service)
            .with_athlete_summary_service(athlete_summary_service)
            .with_llm_services(llm_adapter, llm_config_provider)
            .with_workout_summary_service(workout_summary_service)
            .with_intervals_service(intervals_service)
            .with_race_service(race_service)
            .with_intervals_connection_tester(Arc::new(intervals_connection_tester)),
    );
    let listener = TcpListener::bind(address).await?;
    spawn_provider_polling_loop(provider_polling_service);

    let serve_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    let telemetry_shutdown_result = telemetry.shutdown();

    finish_server_shutdown(serve_result, telemetry_shutdown_result)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
