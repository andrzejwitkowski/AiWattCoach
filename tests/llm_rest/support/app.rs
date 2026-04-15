use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::{
        llm::{
            adapter::LlmAdapter, gemini::client::GeminiClient, openai::client::OpenAiClient,
            openrouter::client::OpenRouterClient, settings_adapter::SettingsLlmConfigProvider,
            workout_summary_coach::LlmWorkoutCoach,
        },
        support::{SystemClock, UuidIdGenerator},
    },
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        athlete_summary::AthleteSummary, settings::UserSettingsService,
        training_context::DefaultTrainingContextBuilder, workout_summary::WorkoutSummaryService,
    },
    Settings,
};
use axum::body::to_bytes;
use mongodb::Client;

use super::{
    identity::TestIdentityServiceWithSession,
    in_memory::{
        canonical_completed_workout_from_activity, sample_activity, sample_summary,
        sample_user_settings, InMemoryAthleteSummaryService, InMemoryCoachReplyOperationRepository,
        InMemoryCompletedWorkoutRepository, InMemoryIntervalsService,
        InMemoryLlmContextCacheRepository, InMemoryPlannedWorkoutRepository,
        InMemorySpecialDayRepository, InMemoryUserSettingsRepository,
        InMemoryWorkoutSummaryRepository,
    },
    server::TestLlmUpstreamServer,
};

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 8 * 1024;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct LlmRestTestContext {
    pub(crate) app: axum::Router,
    pub(crate) server: TestLlmUpstreamServer,
    settings_repository: InMemoryUserSettingsRepository,
    summary_repository: InMemoryWorkoutSummaryRepository,
    athlete_summary_service: InMemoryAthleteSummaryService,
    intervals_service: InMemoryIntervalsService,
    completed_workout_repository: InMemoryCompletedWorkoutRepository,
    _fixture: FrontendFixture,
}

impl LlmRestTestContext {
    pub(crate) fn session_cookie(&self, value: &str) -> axum::http::HeaderValue {
        axum::http::HeaderValue::from_str(&format!("aiwattcoach_session={value}; Path=/")).unwrap()
    }

    pub(crate) fn seed_user_settings(&self, settings: aiwattcoach::domain::settings::UserSettings) {
        self.settings_repository.seed(settings);
    }

    pub(crate) fn seed_summary(
        &self,
        summary: aiwattcoach::domain::workout_summary::WorkoutSummary,
    ) {
        self.summary_repository.seed(summary);
    }

    pub(crate) fn default_settings(&self) -> aiwattcoach::domain::settings::UserSettings {
        sample_user_settings()
    }

    pub(crate) fn default_summary(
        &self,
        workout_id: &str,
    ) -> aiwattcoach::domain::workout_summary::WorkoutSummary {
        sample_summary(workout_id)
    }

    pub(crate) fn seed_athlete_summary(
        &self,
        user_id: &str,
        summary: Option<AthleteSummary>,
        stale: bool,
    ) {
        self.athlete_summary_service.seed(user_id, summary, stale);
    }

    pub(crate) fn seed_activity(&self, activity: aiwattcoach::domain::intervals::Activity) {
        self.intervals_service
            .seed_activities(vec![activity.clone()]);
        self.completed_workout_repository
            .seed(vec![canonical_completed_workout_from_activity(&activity)]);
    }

    pub(crate) fn default_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> aiwattcoach::domain::intervals::Activity {
        sample_activity(user_id, activity_id)
    }
}

pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
    response: axum::response::Response,
) -> T {
    let parts = response.into_parts();
    let body = to_bytes(parts.1, RESPONSE_LIMIT_BYTES)
        .await
        .expect("body to be collected");
    serde_json::from_slice(&body).expect("valid JSON")
}

pub(crate) async fn llm_rest_test_context() -> LlmRestTestContext {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let server = TestLlmUpstreamServer::start().await;

    let settings_repository = InMemoryUserSettingsRepository::default();
    let cache_repository = InMemoryLlmContextCacheRepository::default();
    let summary_repository = InMemoryWorkoutSummaryRepository::default();
    let reply_operation_repository = InMemoryCoachReplyOperationRepository::default();
    let athlete_summary_service = InMemoryAthleteSummaryService::default();
    let intervals_service = InMemoryIntervalsService::default();
    let completed_workout_repository = InMemoryCompletedWorkoutRepository::default();
    let planned_workout_repository = InMemoryPlannedWorkoutRepository;
    let special_day_repository = InMemorySpecialDayRepository;

    let settings_service = Arc::new(
        UserSettingsService::new(settings_repository.clone(), SystemClock)
            .with_llm_context_cache_repository(Arc::new(cache_repository.clone())),
    );
    let llm_config_provider = Arc::new(SettingsLlmConfigProvider::new(settings_service.clone()));
    let llm_http_client = reqwest::Client::new();
    let llm_adapter = Arc::new(LlmAdapter::live(
        OpenAiClient::new(llm_http_client.clone()).with_base_url(server.openai_base_url()),
        GeminiClient::new(llm_http_client.clone()).with_base_url(server.gemini_base_url()),
        OpenRouterClient::new(llm_http_client).with_base_url(server.openrouter_base_url()),
    ));
    let workout_summary_service = Arc::new(
        WorkoutSummaryService::with_coach(
            summary_repository.clone(),
            reply_operation_repository,
            SystemClock,
            UuidIdGenerator,
            Arc::new(
                {
                    let training_context_builder = Arc::new(
                        DefaultTrainingContextBuilder::new(
                            settings_service.clone(),
                            Arc::new(summary_repository.clone()),
                            SystemClock,
                        )
                        .with_completed_workout_repository(completed_workout_repository.clone())
                        .with_planned_workout_repository(planned_workout_repository.clone())
                        .with_special_day_repository(special_day_repository.clone()),
                    );
                    LlmWorkoutCoach::new(
                        llm_adapter.clone(),
                        llm_config_provider.clone(),
                        training_context_builder,
                        SystemClock,
                    )
                }
                .with_context_cache_repository(Arc::new(cache_repository)),
            ),
        )
        .with_athlete_summary_service(Arc::new(athlete_summary_service.clone()))
        .with_settings_service(settings_service.clone()),
    );

    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(
            Arc::new(TestIdentityServiceWithSession),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_settings_service(settings_service)
        .with_athlete_summary_service(Arc::new(athlete_summary_service.clone()))
        .with_llm_services(llm_adapter, llm_config_provider)
        .with_workout_summary_service(workout_summary_service),
        fixture.dist_dir(),
    );

    LlmRestTestContext {
        app,
        server,
        settings_repository,
        summary_repository,
        athlete_summary_service,
        intervals_service,
        completed_workout_repository,
        _fixture: fixture,
    }
}

struct FrontendFixture {
    root: PathBuf,
}

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-llm-rest-spa-fixture-{}-{unique}-{counter}",
        std::process::id()
    ));
    let dist_dir = root.join("dist");
    fs::create_dir_all(&dist_dir).unwrap();
    fs::write(
        dist_dir.join("index.html"),
        "<!doctype html><html><body><div id=\"root\">fixture</div></body></html>",
    )
    .unwrap();

    FrontendFixture { root }
}

impl FrontendFixture {
    fn dist_dir(&self) -> PathBuf {
        self.root.join("dist")
    }
}

impl Drop for FrontendFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}
