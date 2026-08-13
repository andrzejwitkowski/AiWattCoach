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
            get_selected_workout_data::GetSelectedWorkoutDataAdapter,
            meso_cycle_generator::MesoCycleLlmGenerator,
            meso_cycle_llm_config::MesoCycleLlmConfigProvider,
            openai_compatible::client::OpenAiCompatibleClient,
            openrouter::client::OpenRouterClient, settings_adapter::SettingsLlmConfigProvider,
            training_plan_generator::TrainingPlanLlmGenerator,
            update_planned_workout_data::UpdatePlannedWorkoutDataAdapter,
            workout_llm_config::WorkoutLlmConfigProvider, workout_summary_coach::LlmWorkoutCoach,
            zai::client::ZaiClient,
        },
        mongo::{
            activities::MongoActivityRepository,
            activity_upload_operations::MongoActivityUploadOperationRepository,
            athlete_summary::MongoAthleteSummaryRepository,
            athlete_summary_generation_operations::MongoAthleteSummaryGenerationOperationRepository,
            calendar_entry_view_calendar::MongoCalendarEntryViewCalendarSource,
            calendar_entry_views::MongoCalendarEntryViewRepository,
            calendar_planned_workouts::MongoCalendarPlannedWorkoutSource,
            client::{create_client, ensure_database_exists, verify_connection},
            coach_conversation_messages::MongoCoachConversationMessageRepository,
            coach_conversations::MongoCoachConversationRepository,
            completed_workouts::MongoCompletedWorkoutRepository,
            external_observations::MongoExternalObservationRepository,
            external_sync_states::MongoExternalSyncStateRepository,
            ftp_history::MongoFtpHistoryRepository,
            llm_context_cache::MongoLlmContextCacheRepository,
            llm_reply_operations::MongoLlmReplyOperationRepository,
            login_state::MongoLoginStateRepository,
            meso_cycle_generation_operations::MongoMesoCycleGenerationOperationRepository,
            meso_cycle_projections::MongoMesoCycleProjectionRepository,
            planned_completed_links::MongoPlannedCompletedWorkoutLinkRepository,
            planned_rest_days::MongoPlannedRestDayRepository,
            planned_rest_days_calendar::MongoPlannedRestDayCalendarLabelSource,
            planned_workout_tokens::MongoPlannedWorkoutTokenRepository,
            planned_workouts::MongoPlannedWorkoutRepository,
            provider_poll_states::MongoProviderPollStateRepository,
            races::MongoRaceRepository,
            readable_dates_backfill::backfill_mongo_readable_dates,
            sessions::MongoSessionRepository,
            settings::MongoUserSettingsRepository,
            special_days::MongoSpecialDayRepository,
            task_workers::MongoTaskWorkerRepository,
            tasks::MongoTaskRepository,
            training_load_daily_snapshots::MongoTrainingLoadDailySnapshotRepository,
            training_plan_generation_operations::MongoTrainingPlanGenerationOperationRepository,
            training_plan_projections::MongoTrainingPlanProjectionRepository,
            training_plan_snapshots::MongoTrainingPlanSnapshotRepository,
            users::MongoUserRepository,
            wahoo_connect_state::MongoWahooConnectStateRepository,
            wahoo_fit_files::MongoWahooFitFileRepository,
            whitelist::MongoWhitelistRepository,
            workout_summary::MongoWorkoutSummaryRepository,
        },
        support::{SystemClock, UuidIdGenerator},
        wahoo::{
            adapter::WahooOAuthAdapter, client::WahooOAuthClient, dev_client::DevWahooOAuthClient,
        },
        wahoo_fit_parser::WahooFitParser,
        workout_summary_completed_target::CompletedWorkoutTargetAdapter,
        workout_summary_latest_activity::LatestCompletedActivityAdapter,
    },
    build_app,
    config::{
        default_task_scheduler_worker_id, spawn_provider_polling_loop,
        spawn_task_scheduler_maintenance_loop, spawn_task_worker,
        workout_summary_task_worker_config, ProviderPollingService, Settings,
        TaskSchedulerMaintenanceConfig, TaskSchedulerWorkerConfig,
    },
    domain::admin_prompt_preview::{AdminPromptPreviewService, AdminPromptPreviewUseCases},
    domain::athlete_summary::{
        athlete_summary_generate_task_handler, AthleteSummaryService,
        SchedulerBackedAthleteSummaryService,
    },
    domain::calendar::CalendarService,
    domain::calendar_coach::SharedCalendarCoachService,
    domain::calendar_labels::{CalendarLabelsService, CompositeCalendarLabelSource},
    domain::calendar_view::{CalendarEntryViewRefreshService, ManualCalendarRefreshService},
    domain::coach_conversation::{
        coach_conversation_reply_task_handler, SchedulerBackedCoachConversationService,
        SharedCoachConversationService,
    },
    domain::completed_workouts::{
        AuthoritativeCompletedWorkoutRepository, CompletedWorkoutReadService,
        PowerCurveCompletedWorkoutRepository,
    },
    domain::external_sync::ExternalImportService,
    domain::identity::{
        validate_session_ttl_against_current_time, Clock, IdentityService, IdentityServiceConfig,
        IdentityServiceDependencies,
    },
    domain::intervals::IntervalsService,
    domain::meso_cycle::{
        meso_cycle_generate_task_handler, MesoCycleService, MesoCycleWindowPort,
        SchedulerBackedMesoCycleService, TrainingPlanBackedMesoWindowPort,
    },
    domain::planned_rest_days::PlannedRestDayService,
    domain::planned_workouts::AuthoritativePlannedWorkoutRepository,
    domain::races::{AuthoritativeRaceRepository, RaceService},
    domain::settings::UserSettingsService,
    domain::special_days::AuthoritativeSpecialDayRepository,
    domain::task_scheduler::TaskSchedulerService,
    domain::training_context::DefaultTrainingContextBuilder,
    domain::training_load::{TrainingLoadDashboardReadService, TrainingLoadRecomputeService},
    domain::training_plan::{
        training_plan_generate_task_handler, RaceProjectionCleanupService,
        SchedulerBackedTrainingPlanService, TrainingPlanGenerationService,
    },
    domain::wahoo::{WahooService, WahooWebhookService},
    domain::wahoo_fit_enrichment::{
        wahoo_fit_enrichment_task_handler, SchedulerBackedWahooFitEnrichmentService,
        WahooFitEnrichmentService,
    },
    domain::workout_summary::{
        workout_summary_coach_reply_task_handler, SchedulerBackedWorkoutSummaryService,
        WorkoutSummaryService,
    },
    main_runtime::{
        finish_server_shutdown, park_wahoo_poll_states, reconcile_intervals_poll_states,
        reconcile_wahoo_user_ids, shutdown_signal, TrainingPlanWorkoutSummaryAdapter,
    },
    telemetry::setup_telemetry,
    AppState,
};
use tokio::net::TcpListener;
use tracing::info;

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
    let wahoo_connect_state_repository =
        MongoWahooConnectStateRepository::new(mongo_client.clone(), &mongo_database);
    user_repository.ensure_indexes().await?;
    session_repository.ensure_indexes().await?;
    login_state_repository.ensure_indexes().await?;
    whitelist_repository.ensure_indexes().await?;
    wahoo_connect_state_repository.ensure_indexes().await?;
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
    let task_repository = MongoTaskRepository::new(mongo_client.clone(), &mongo_database);
    task_repository.ensure_indexes().await?;
    let task_worker_repository =
        MongoTaskWorkerRepository::new(mongo_client.clone(), &mongo_database);
    task_worker_repository.ensure_indexes().await?;
    let shared_task_scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        task_worker_repository.clone(),
        SystemClock,
    );
    let llm_http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(180))
        .build()?;
    let llm_adapter = if dev_llm_coach_enabled {
        Arc::new(LlmAdapter::Dev(DevLlmCoachAdapter))
    } else {
        Arc::new(LlmAdapter::live(
            OpenAiCompatibleClient::new(llm_http_client.clone()),
            OpenAiCompatibleClient::new(llm_http_client.clone())
                .with_base_url("https://api.deepseek.com"),
            ZaiClient::new(llm_http_client.clone()),
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
    let athlete_summary_repository_for_admin = athlete_summary_repository.clone();
    let athlete_summary_generation_operation_repository =
        MongoAthleteSummaryGenerationOperationRepository::new(
            mongo_client.clone(),
            &mongo_database,
        );
    athlete_summary_generation_operation_repository
        .ensure_indexes()
        .await?;
    let coach_reply_operation_repository = MongoLlmReplyOperationRepository::new(
        mongo_client.clone(),
        &mongo_database,
        "workout_summary",
    );
    let coach_conversation_reply_operation_repository = MongoLlmReplyOperationRepository::new(
        mongo_client.clone(),
        &mongo_database,
        "coach_conversation",
    );
    coach_reply_operation_repository.ensure_indexes().await?;
    let coach_conversation_repository =
        MongoCoachConversationRepository::new(mongo_client.clone(), &mongo_database);
    coach_conversation_repository.ensure_indexes().await?;
    let coach_conversation_message_repository =
        MongoCoachConversationMessageRepository::new(mongo_client.clone(), &mongo_database);
    coach_conversation_message_repository
        .ensure_indexes()
        .await?;
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
    let meso_cycle_generation_operation_repository =
        MongoMesoCycleGenerationOperationRepository::new(mongo_client.clone(), &mongo_database);
    meso_cycle_generation_operation_repository
        .ensure_indexes()
        .await?;
    let meso_cycle_projection_repository =
        MongoMesoCycleProjectionRepository::new(mongo_client.clone(), &mongo_database);
    meso_cycle_projection_repository.ensure_indexes().await?;
    let meso_cycle_projection_repository_for_coach = meso_cycle_projection_repository.clone();
    let training_plan_window_port_ops = training_plan_generation_operation_repository.clone();
    let training_plan_window_port_projections = training_plan_projection_repository.clone();
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
    park_wahoo_poll_states(&provider_poll_state_repository).await?;
    let race_repository = MongoRaceRepository::new(mongo_client.clone(), &mongo_database);
    race_repository.ensure_indexes().await?;
    let planned_rest_day_repository =
        MongoPlannedRestDayRepository::new(mongo_client.clone(), &mongo_database);
    planned_rest_day_repository.ensure_indexes().await?;
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
    let completed_workout_repository =
        PowerCurveCompletedWorkoutRepository::new(completed_workout_repository);
    let wahoo_fit_file_repository =
        MongoWahooFitFileRepository::new(mongo_client.clone(), &mongo_database);
    wahoo_fit_file_repository.ensure_indexes().await?;
    let run_readable_dates_backfill = matches!(
        std::env::var("RUN_MONGO_READABLE_DATES_BACKFILL").as_deref(),
        Ok("true")
    );
    if run_readable_dates_backfill {
        let readable_date_backfilled_documents =
            backfill_mongo_readable_dates(&mongo_client, &mongo_database).await?;
        if readable_date_backfilled_documents > 0 {
            info!(
                readable_date_backfilled_documents,
                "Backfilled readable Mongo BSON DateTime mirrors"
            );
        }
    } else {
        info!(
            "Skipping readable Mongo BSON DateTime mirror backfill; set RUN_MONGO_READABLE_DATES_BACKFILL=true to run it"
        );
    }
    let authoritative_completed_workout_repository = AuthoritativeCompletedWorkoutRepository::new(
        completed_workout_repository.clone(),
        external_sync_state_repository.clone(),
    );
    let authoritative_planned_workout_repository = AuthoritativePlannedWorkoutRepository::new(
        planned_workout_repository.clone(),
        authoritative_completed_workout_repository.clone(),
        planned_completed_link_repository.clone(),
    );
    let training_load_recompute_service = Arc::new(TrainingLoadRecomputeService::new(
        authoritative_completed_workout_repository.clone(),
        ftp_history_repository.clone(),
        training_load_daily_snapshot_repository.clone(),
        settings_repository.clone(),
    ));
    let training_load_dashboard_service = Arc::new(TrainingLoadDashboardReadService::new(
        training_load_daily_snapshot_repository.clone(),
    ));
    let settings_service = Arc::new(
        UserSettingsService::new(settings_repository.clone(), SystemClock)
            .with_provider_poll_states(provider_poll_state_repository.clone())
            .with_llm_context_cache_repository(Arc::new(llm_context_cache_repository.clone()))
            .with_ftp_history_repository(ftp_history_repository.clone())
            .with_training_load_recompute_service(training_load_recompute_service.clone()),
    );
    let wahoo_service = match auth.wahoo.clone() {
        Some(wahoo) => {
            let oauth = if auth.dev.enabled {
                WahooOAuthAdapter::Dev(DevWahooOAuthClient)
            } else {
                WahooOAuthAdapter::Live(WahooOAuthClient::new(
                    reqwest::Client::builder()
                        .connect_timeout(Duration::from_secs(5))
                        .timeout(Duration::from_secs(15))
                        .build()?,
                    wahoo.client_id,
                    wahoo.client_secret,
                    wahoo.redirect_url,
                    wahoo.authorize_url,
                    wahoo.token_url,
                    wahoo.scope,
                ))
            };

            Some(Arc::new(WahooService::new(
                settings_repository.clone(),
                wahoo_connect_state_repository.clone(),
                oauth,
                SystemClock,
                UuidIdGenerator,
            ))
                as Arc<dyn aiwattcoach::domain::wahoo::WahooUseCases>)
        }
        None => None,
    };
    if let Some(wahoo_service) = wahoo_service.as_ref() {
        reconcile_wahoo_user_ids(&settings_repository, wahoo_service.as_ref(), &SystemClock)
            .await?;
    }
    let llm_config_provider = Arc::new(SettingsLlmConfigProvider::new(settings_service.clone()));
    let workout_llm_config_provider =
        Arc::new(WorkoutLlmConfigProvider::new(settings_service.clone()));
    let special_day_repository =
        MongoSpecialDayRepository::new(mongo_client.clone(), &mongo_database);
    special_day_repository.ensure_indexes().await?;
    let authoritative_special_day_repository = AuthoritativeSpecialDayRepository::new(
        special_day_repository.clone(),
        external_observation_repository.clone(),
    );
    let authoritative_race_repository = AuthoritativeRaceRepository::new(
        race_repository.clone(),
        external_observation_repository.clone(),
    );
    let get_selected_workout_data_port = Arc::new(GetSelectedWorkoutDataAdapter {
        completed: authoritative_completed_workout_repository.clone(),
        planned: authoritative_planned_workout_repository.clone(),
        races: authoritative_race_repository.clone(),
        summaries: workout_summary_repository.clone(),
    });
    let calendar_entry_view_repository =
        MongoCalendarEntryViewRepository::new(mongo_client.clone(), &mongo_database);
    calendar_entry_view_repository.ensure_indexes().await?;
    let calendar_planned_workout_source =
        MongoCalendarPlannedWorkoutSource::new(mongo_client.clone(), &mongo_database);
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
        calendar_planned_workout_source.clone(),
        authoritative_completed_workout_repository.clone(),
        authoritative_race_repository.clone(),
        authoritative_special_day_repository.clone(),
        external_sync_state_repository.clone(),
    )
    .with_cleanup_planned_workouts(calendar_planned_workout_source.clone())
    .with_planned_completed_links(planned_completed_link_repository.clone());
    let race_projection_cleanup =
        RaceProjectionCleanupService::new(training_plan_projection_repository.clone(), SystemClock);
    let manual_calendar_refresh_service = Arc::new(
        ManualCalendarRefreshService::new(
            calendar_entry_view_repository.clone(),
            calendar_planned_workout_source.clone(),
            authoritative_completed_workout_repository.clone(),
            authoritative_race_repository.clone(),
            authoritative_special_day_repository.clone(),
            SystemClock,
            calendar_entry_view_refresh_service.clone(),
        )
        .with_orphan_race_projection_cleanup(race_projection_cleanup.clone()),
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
    let wahoo_fit_enrichment_service = wahoo_service.clone().map(|wahoo_service| {
        Arc::new(
            WahooFitEnrichmentService::new(
                wahoo_service,
                completed_workout_repository.clone(),
                wahoo_fit_file_repository.clone(),
                WahooFitParser,
                SystemClock,
            )
            .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone())
            .with_training_load_recompute_service(training_load_recompute_service.clone()),
        )
    });
    let wahoo_fit_enrichment_queue_service = wahoo_fit_enrichment_service.clone().map(|_| {
        Arc::new(SchedulerBackedWahooFitEnrichmentService::new(
            shared_task_scheduler.clone(),
            UuidIdGenerator,
        ))
    });
    let wahoo_webhook_service = match (
        wahoo_service.clone(),
        wahoo_fit_enrichment_queue_service.clone(),
    ) {
        (Some(wahoo_service), Some(queue)) => Some(Arc::new(WahooWebhookService::new(
            settings_repository.clone(),
            external_import_service.clone(),
            wahoo_service,
            (*training_load_recompute_service).clone(),
            (*queue).clone(),
            SystemClock,
            auth.wahoo
                .as_ref()
                .and_then(|settings| settings.webhook_token.clone()),
        ))),
        _ => None,
    };
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
    let planned_workout_update_service = Arc::new(
        aiwattcoach::domain::planned_workouts::PlannedWorkoutUpdateService::new(
            planned_workout_repository.clone(),
            external_sync_state_repository.clone(),
            (*intervals_service).clone(),
            wahoo_service
                .clone()
                .unwrap_or_else(|| Arc::new(aiwattcoach::domain::calendar::NoopWahooUseCases)),
            settings_repository.clone(),
            planned_workout_token_repository.clone(),
            calendar_entry_view_refresh_service.clone(),
            SystemClock,
        ),
    );
    let planned_workout_update_port = Arc::new(UpdatePlannedWorkoutDataAdapter::new(
        (*planned_workout_update_service).clone(),
    ));

    let completed_workout_target_service = Arc::new(CompletedWorkoutTargetAdapter::new(
        authoritative_completed_workout_repository.clone(),
    ));

    let training_context_builder = Arc::new(
        DefaultTrainingContextBuilder::new(
            settings_service.clone(),
            Arc::new(workout_summary_repository.clone()),
            completed_workout_target_service.clone(),
            SystemClock,
        )
        .with_completed_workout_repository(authoritative_completed_workout_repository.clone())
        .with_planned_workout_repository(authoritative_planned_workout_repository.clone())
        .with_unfiltered_planned_workout_repository(planned_workout_repository.clone())
        .with_special_day_repository(authoritative_special_day_repository.clone())
        .with_ftp_history_repository(ftp_history_repository.clone())
        .with_training_load_daily_snapshot_repository(
            training_load_daily_snapshot_repository.clone(),
        )
        .with_race_repository(Arc::new(authoritative_race_repository.clone()))
        .with_planned_rest_day_repository(Arc::new(planned_rest_day_repository.clone()))
        .with_training_plan_projection_repository(Arc::new(
            training_plan_projection_repository.clone(),
        )),
    );
    let athlete_summary_direct_service = Arc::new(AthleteSummaryService::new(
        athlete_summary_repository,
        athlete_summary_generation_operation_repository,
        AthleteSummaryLlmGenerator::new(
            llm_adapter.clone(),
            llm_config_provider.clone(),
            training_context_builder.clone(),
            SystemClock,
        ),
        SystemClock,
    ));

    let workout_summary_direct_service = Arc::new(
        WorkoutSummaryService::with_coach(
            workout_summary_repository.clone(),
            coach_reply_operation_repository.clone(),
            SystemClock,
            UuidIdGenerator,
            Arc::new(
                LlmWorkoutCoach::new(
                    llm_adapter.clone(),
                    workout_llm_config_provider.clone(),
                    training_context_builder.clone(),
                    SystemClock,
                )
                .with_context_cache_repository(Arc::new(llm_context_cache_repository.clone()))
                .with_data_port(get_selected_workout_data_port.clone())
                .with_meso_projection_repository(Arc::new(
                    meso_cycle_projection_repository_for_coach.clone(),
                )),
            ),
        )
        .with_athlete_summary_service(athlete_summary_direct_service.clone())
        .with_settings_service(settings_service.clone())
        .with_completed_workout_target_service(completed_workout_target_service)
        .with_latest_completed_activity_service(Arc::new(
            LatestCompletedActivityAdapter::new(authoritative_completed_workout_repository.clone()),
        )),
    );
    let coach_conversation_direct_service = Arc::new(
        SharedCoachConversationService::new(
            coach_conversation_repository,
            coach_conversation_message_repository,
            coach_conversation_reply_operation_repository,
            llm_adapter.clone(),
            llm_config_provider.clone(),
            training_context_builder.clone(),
            SystemClock,
            UuidIdGenerator,
        )
        .with_settings_service(settings_service.clone())
        .with_context_cache_repository(Arc::new(llm_context_cache_repository.clone()))
        .with_data_port(get_selected_workout_data_port.clone())
        .with_planned_workout_update_port(planned_workout_update_port.clone()),
    );
    let training_plan_direct_service = Arc::new(
        TrainingPlanGenerationService::new(
            training_plan_snapshot_repository,
            training_plan_projection_repository.clone(),
            training_plan_generation_operation_repository,
            TrainingPlanLlmGenerator::new(
                llm_adapter.clone(),
                workout_llm_config_provider.clone(),
                training_context_builder.clone(),
                SystemClock,
            )
            .with_data_port(get_selected_workout_data_port.clone()),
            TrainingPlanWorkoutSummaryAdapter::new(workout_summary_direct_service.clone()),
            SystemClock,
        )
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );
    let training_plan_service = Arc::new(SchedulerBackedTrainingPlanService::new(
        training_plan_direct_service.clone(),
        shared_task_scheduler.clone(),
        UuidIdGenerator,
    ));
    let race_service = Arc::new(
        RaceService::new(
            authoritative_race_repository.clone(),
            (*intervals_service).clone(),
            external_sync_state_repository.clone(),
            SystemClock,
            UuidIdGenerator,
        )
        .with_provider_poll_states(provider_poll_state_repository.clone())
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone())
        .with_projection_cleanup(race_projection_cleanup.clone()),
    );
    let race_calendar_source =
        MongoCalendarEntryViewCalendarSource::new(mongo_client.clone(), &mongo_database);
    let planned_rest_day_calendar_source =
        MongoPlannedRestDayCalendarLabelSource::new(planned_rest_day_repository.clone());
    let calendar_labels_service = Arc::new(CalendarLabelsService::new(
        CompositeCalendarLabelSource::new(
            race_calendar_source.clone(),
            planned_rest_day_calendar_source,
        ),
    ));
    let planned_rest_day_service = Arc::new(PlannedRestDayService::new(
        planned_rest_day_repository.clone(),
        SystemClock,
        UuidIdGenerator,
    ));
    let completed_workout_service = Arc::new(CompletedWorkoutReadService::new(
        authoritative_completed_workout_repository.clone(),
    ));
    let completed_workout_admin_service = Arc::new(
        IntervalsCompletedWorkoutBackfillService::new(
            completed_workout_repository.clone(),
            intervals_settings_provider.clone(),
            intervals_api_client.clone(),
            external_import_service.clone(),
            SystemClock,
        )
        .with_training_load_recompute_service(training_load_recompute_service.clone()),
    );
    let calendar_service = Arc::new(
        CalendarService::new(
            (*intervals_service).clone(),
            calendar_entry_view_repository.clone(),
            training_plan_projection_repository.clone(),
            external_sync_state_repository.clone(),
            SystemClock,
        )
        .with_wahoo(
            wahoo_service
                .clone()
                .unwrap_or_else(|| Arc::new(aiwattcoach::domain::calendar::NoopWahooUseCases)),
            settings_repository.clone(),
        )
        .with_planned_workout_tokens(planned_workout_token_repository)
        .with_planned_workouts(authoritative_planned_workout_repository.clone())
        .with_completed_workouts(authoritative_completed_workout_repository.clone())
        .with_calendar_view_refresh(calendar_entry_view_refresh_service.clone()),
    );
    let save_notifier = Arc::new(aiwattcoach::adapters::rest::WorkoutSummarySaveNotifier::new());
    let workout_summary_direct_service = Arc::new(
        (*workout_summary_direct_service)
            .clone()
            .with_training_plan_service(training_plan_service.clone())
            .with_save_completion_port(save_notifier.clone()),
    );
    let athlete_summary_service = Arc::new(SchedulerBackedAthleteSummaryService::new(
        athlete_summary_direct_service.clone(),
        shared_task_scheduler.clone(),
        UuidIdGenerator,
    ));
    let meso_cycle_llm_config_provider =
        Arc::new(MesoCycleLlmConfigProvider::new(settings_service.clone()));
    let admin_meso_window_port: Arc<dyn MesoCycleWindowPort> =
        Arc::new(TrainingPlanBackedMesoWindowPort::new(
            training_plan_window_port_ops.clone(),
            training_plan_window_port_projections.clone(),
        ));
    let meso_cycle_direct_service = Arc::new(MesoCycleService::new(
        meso_cycle_generation_operation_repository,
        meso_cycle_projection_repository,
        MesoCycleLlmGenerator::new(
            llm_adapter.clone(),
            meso_cycle_llm_config_provider.clone(),
            training_context_builder.clone(),
            get_selected_workout_data_port.clone(),
            SystemClock,
        ),
        TrainingPlanBackedMesoWindowPort::new(
            training_plan_window_port_ops,
            training_plan_window_port_projections,
        ),
        SystemClock,
    ));
    let meso_cycle_service = Arc::new(SchedulerBackedMesoCycleService::new(
        meso_cycle_direct_service.clone(),
        shared_task_scheduler.clone(),
        UuidIdGenerator,
    ));
    let calendar_coach_service = Arc::new(SharedCalendarCoachService::new(Arc::new(
        SchedulerBackedCoachConversationService::new(
            coach_conversation_direct_service.clone(),
            shared_task_scheduler.clone(),
            UuidIdGenerator,
        ),
    )));
    let mut task_handlers = vec![
        workout_summary_coach_reply_task_handler(workout_summary_direct_service.clone()),
        coach_conversation_reply_task_handler(coach_conversation_direct_service),
        athlete_summary_generate_task_handler(athlete_summary_direct_service.clone()),
        training_plan_generate_task_handler(training_plan_direct_service),
        meso_cycle_generate_task_handler(meso_cycle_direct_service.clone()),
    ];
    if let Some(service) = wahoo_fit_enrichment_service.clone() {
        task_handlers.push(wahoo_fit_enrichment_task_handler(service));
    }
    let workout_summary_task_worker = spawn_task_worker(
        shared_task_scheduler.clone(),
        format!("{}-workout-summary", default_task_scheduler_worker_id()),
        workout_summary_task_worker_config(),
        task_handlers,
    )?;
    let workout_summary_service = Arc::new(SchedulerBackedWorkoutSummaryService::new(
        workout_summary_direct_service,
        shared_task_scheduler.clone(),
        UuidIdGenerator,
    ));
    let admin_prompt_preview_service: Arc<dyn AdminPromptPreviewUseCases> =
        Arc::new(AdminPromptPreviewService::new(
            training_context_builder.clone(),
            llm_config_provider.clone(),
            workout_llm_config_provider.clone(),
            workout_llm_config_provider.clone(),
            completed_workout_service.clone(),
            Some(planned_workout_repository.clone()),
            Some(special_day_repository.clone()),
            Arc::new(CompletedWorkoutTargetAdapter::new(
                authoritative_completed_workout_repository.clone(),
            )),
            Arc::new(workout_summary_repository.clone()),
            Some(Arc::new(athlete_summary_repository_for_admin)),
            settings_service.clone(),
            Some(get_selected_workout_data_port.clone()),
            Some(planned_workout_update_port.clone()),
            Some(admin_meso_window_port),
            Some(meso_cycle_llm_config_provider),
            Some(Arc::new(meso_cycle_projection_repository_for_coach)),
            SystemClock,
        ));

    let intervals_connection_tester = if dev_intervals_enabled {
        IntervalsApiAdapter::Dev(DevIntervalsClient)
    } else {
        IntervalsApiAdapter::Live(IntervalsIcuClient::with_timeouts(5, 15)?)
    };

    let app_state = AppState::new(app_name, mongo_database, mongo_client)
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
        .with_admin_task_scheduler_service(Arc::new(shared_task_scheduler.clone()))
        .with_admin_prompt_preview_service(admin_prompt_preview_service)
        .with_training_load_dashboard_service(training_load_dashboard_service)
        .with_calendar_service(calendar_service)
        .with_calendar_coach_service(calendar_coach_service)
        .with_calendar_labels_service(calendar_labels_service)
        .with_manual_calendar_refresh_service(manual_calendar_refresh_service)
        .with_completed_workout_service(completed_workout_service)
        .with_completed_workout_admin_service(completed_workout_admin_service)
        .with_athlete_summary_service(athlete_summary_service)
        .with_meso_cycle_service(meso_cycle_service)
        .with_llm_services(llm_adapter, llm_config_provider)
        .with_workout_summary_service(workout_summary_service)
        .with_workout_summary_save_notifier((*save_notifier).clone())
        .with_intervals_service(intervals_service)
        .with_race_service(race_service)
        .with_planned_rest_day_service(planned_rest_day_service)
        .with_intervals_connection_tester(Arc::new(intervals_connection_tester));
    let app_state = if let Some(wahoo_service) = wahoo_service {
        let app_state = app_state.with_wahoo_service(wahoo_service);
        if let Some(wahoo_webhook_service) = wahoo_webhook_service {
            app_state.with_wahoo_webhook_service(wahoo_webhook_service)
        } else {
            app_state
        }
    } else {
        app_state
    };
    let app = build_app(app_state);
    let listener = TcpListener::bind(address).await?;
    let provider_polling_loop = spawn_provider_polling_loop(provider_polling_service);
    // Prefer a stable worker id from env or container hostname so a process restart can be
    // recognized as the same logical worker. If neither exists, fall back to a per-process id.
    let task_scheduler_maintenance_loop = spawn_task_scheduler_maintenance_loop(
        shared_task_scheduler.clone(),
        TaskSchedulerWorkerConfig::new(default_task_scheduler_worker_id(), false, Vec::new()),
        TaskSchedulerMaintenanceConfig::default(),
    )?;

    let serve_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    workout_summary_task_worker.shutdown().await;
    provider_polling_loop.shutdown().await;
    task_scheduler_maintenance_loop.shutdown().await;
    let telemetry_shutdown_result = telemetry.shutdown();

    finish_server_shutdown(serve_result, telemetry_shutdown_result)
}
