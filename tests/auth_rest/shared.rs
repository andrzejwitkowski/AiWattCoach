use std::{
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::{AppState, WhitelistRateLimiter},
    domain::admin_prompt_preview::AdminPromptPreviewUseCases,
    domain::identity::{
        AppUser, AuthSession, GoogleLoginOutcome, GoogleLoginStart, GoogleLoginSuccess,
        IdentityError, IdentityUseCases, Role, WhitelistEntry,
    },
    domain::settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsUseCases,
    },
    domain::task_scheduler::AdminTaskSchedulerUseCases,
    domain::wahoo::{
        WahooAuthExchange, WahooAuthStart, WahooCreatePlan, WahooCreateWorkout, WahooError,
        WahooPlan, WahooToken, WahooUpdatePlan, WahooUpdateWorkout, WahooUseCases, WahooUser,
        WahooWebhookAccepted, WahooWebhookError, WahooWebhookOutcome, WahooWebhookUseCases,
        WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
    },
    Settings,
};
use mongodb::Client;

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

type BeginConnectInput = (String, Option<String>);
type FinishConnectInput = (String, String, String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WahooWebhookImportCall {
    pub(crate) webhook_token: String,
    pub(crate) wahoo_user_id: i64,
    pub(crate) workout_id: i64,
    pub(crate) starts: String,
    pub(crate) has_workout_summary: bool,
}

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static SHARED_FRONTEND_FIXTURE: OnceLock<FrontendFixture> = OnceLock::new();

pub(crate) async fn auth_test_app(identity_service: TestIdentityService) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_trust_proxy_headers(settings.trust_proxy_headers)
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        ),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_custom_settings(
    settings: Settings,
    identity_service: TestIdentityService,
) -> axum::Router {
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_trust_proxy_headers(settings.trust_proxy_headers)
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            settings.auth.session.cookie_name,
            settings.auth.session.same_site,
            settings.auth.session.secure,
            settings.auth.session.ttl_hours,
        ),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_custom_settings_and_limited_whitelist_rate(
    settings: Settings,
    identity_service: TestIdentityService,
    max_attempts: usize,
) -> axum::Router {
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_trust_proxy_headers(settings.trust_proxy_headers)
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            max_attempts,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            settings.auth.session.cookie_name,
            settings.auth.session.same_site,
            settings.auth.session.secure,
            settings.auth.session.ttl_hours,
        ),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_without_identity() -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_settings(
    identity_service: TestIdentityService,
    settings_service: TestSettingsService,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_settings_service(std::sync::Arc::new(settings_service)),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_admin_prompt_preview(
    identity_service: TestIdentityService,
    admin_prompt_preview_service: impl AdminPromptPreviewUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_trust_proxy_headers(settings.trust_proxy_headers)
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_admin_prompt_preview_service(std::sync::Arc::new(admin_prompt_preview_service)),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_admin_task_scheduler(
    identity_service: TestIdentityService,
    task_scheduler_service: impl AdminTaskSchedulerUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_trust_proxy_headers(settings.trust_proxy_headers)
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_admin_task_scheduler_service(std::sync::Arc::new(task_scheduler_service)),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_wahoo(
    identity_service: TestIdentityService,
    wahoo_service: TestWahooService,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_wahoo_service(std::sync::Arc::new(wahoo_service)),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_wahoo_webhook(
    identity_service: TestIdentityService,
    wahoo_webhook_service: impl WahooWebhookUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            usize::MAX,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_wahoo_webhook_service(std::sync::Arc::new(wahoo_webhook_service)),
        dist_dir,
    )
}

pub(crate) async fn auth_test_app_with_limited_whitelist_rate(
    identity_service: TestIdentityService,
    max_attempts: usize,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let dist_dir = shared_frontend_fixture().dist_dir();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_whitelist_rate_limiter(WhitelistRateLimiter::new(
            max_attempts,
            std::time::Duration::from_secs(60),
        ))
        .with_identity_service(
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        ),
        dist_dir,
    )
}

#[derive(Default)]
pub(crate) struct TestSettingsService;

impl UserSettingsUseCases for TestSettingsService {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        Box::pin(async move { Ok(None) })
    }

    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(UserSettings::new_defaults(user_id, 1000)) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_ai_agents is not used in auth tests") })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_intervals is not used in auth tests") })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_options is not used in auth tests") })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_cycling is not used in auth tests") })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_availability is not used in auth tests") })
    }
}

#[derive(Clone)]
pub(crate) struct TestWahooService {
    pub(crate) begin_result: Result<WahooAuthStart, WahooError>,
    pub(crate) finish_result: Result<WahooAuthExchange, WahooError>,
    pub(crate) ensure_result: Result<WahooToken, WahooError>,
    pub(crate) last_begin_input: Arc<Mutex<Option<BeginConnectInput>>>,
    pub(crate) last_finish_input: Arc<Mutex<Option<FinishConnectInput>>>,
    pub(crate) last_ensure_user_id: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
pub(crate) struct TestWahooWebhookService {
    result: Result<WahooWebhookOutcome, WahooWebhookError>,
    import_calls: Arc<Mutex<Vec<WahooWebhookImportCall>>>,
}

impl TestWahooWebhookService {
    pub(crate) fn accepting() -> Self {
        Self {
            result: Ok(WahooWebhookOutcome::Accepted(WahooWebhookAccepted {
                user_id: "user-1".to_string(),
                completed_workout_id: "wahoo-workout:42".to_string(),
            })),
            import_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn ignored() -> Self {
        Self {
            result: Ok(WahooWebhookOutcome::Ignored),
            import_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn unauthorized() -> Self {
        Self {
            result: Err(WahooWebhookError::Unauthorized),
            import_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn not_configured() -> Self {
        Self {
            result: Err(WahooWebhookError::NotConfigured),
            import_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn import_calls(&self) -> Vec<WahooWebhookImportCall> {
        self.import_calls.lock().unwrap().clone()
    }
}

impl Default for TestWahooService {
    fn default() -> Self {
        Self {
            begin_result: Ok(WahooAuthStart {
                state: "wahoo-state-1".to_string(),
                redirect_url: "https://api.wahooligan.com/oauth/authorize?state=wahoo-state-1"
                    .to_string(),
            }),
            finish_result: Ok(WahooAuthExchange {
                redirect_to: "/settings?connected=wahoo".to_string(),
                token: WahooToken {
                    access_token: "access-token".to_string(),
                    refresh_token: "refresh-token".to_string(),
                    expires_at_epoch_seconds: 1_800_000_000,
                },
            }),
            ensure_result: Ok(WahooToken {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires_at_epoch_seconds: 1_800_000_000,
            }),
            last_begin_input: Arc::new(Mutex::new(None)),
            last_finish_input: Arc::new(Mutex::new(None)),
            last_ensure_user_id: Arc::new(Mutex::new(None)),
        }
    }
}

impl WahooUseCases for TestWahooService {
    fn begin_connect(
        &self,
        user_id: &str,
        return_to: Option<String>,
    ) -> BoxFuture<Result<WahooAuthStart, WahooError>> {
        *self.last_begin_input.lock().unwrap() = Some((user_id.to_string(), return_to));
        let result = self.begin_result.clone();
        Box::pin(async move { result })
    }

    fn finish_connect(
        &self,
        user_id: &str,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<WahooAuthExchange, WahooError>> {
        *self.last_finish_input.lock().unwrap() =
            Some((user_id.to_string(), state.to_string(), code.to_string()));
        let result = self.finish_result.clone();
        Box::pin(async move { result })
    }

    fn ensure_token(&self, user_id: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        *self.last_ensure_user_id.lock().unwrap() = Some(user_id.to_string());
        let result = self.ensure_result.clone();
        Box::pin(async move { result })
    }

    fn get_authenticated_user(&self, _user_id: &str) -> BoxFuture<Result<WahooUser, WahooError>> {
        Box::pin(async move { Ok(WahooUser { id: 60_462 }) })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        _page: usize,
        _per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>> {
        Box::pin(async {
            Ok(WahooWorkoutList {
                workouts: Vec::new(),
                total: 0,
                page: 1,
                per_page: 100,
                order: None,
                sort: None,
            })
        })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotFound) })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> BoxFuture<Result<Option<WahooPlan>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        _request: WahooCreatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        _plan_id: i64,
        _request: WahooUpdatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        _request: WahooCreateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
        _request: WahooUpdateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn download_workout_file(&self, _file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

impl WahooWebhookUseCases for TestWahooWebhookService {
    fn import_webhook_workout(
        &self,
        webhook_token: &str,
        wahoo_user_id: i64,
        workout: WahooWorkout,
    ) -> BoxFuture<Result<WahooWebhookOutcome, WahooWebhookError>> {
        self.import_calls
            .lock()
            .unwrap()
            .push(WahooWebhookImportCall {
                webhook_token: webhook_token.to_string(),
                wahoo_user_id,
                workout_id: workout.id,
                starts: workout.starts.clone(),
                has_workout_summary: workout.workout_summary.is_some(),
            });
        let result = self.result.clone();
        Box::pin(async move { result })
    }

    fn sync_completed_workouts_for_user(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<aiwattcoach::domain::wahoo::ManualWahooSyncResult, WahooWebhookError>>
    {
        Box::pin(async {
            Ok(aiwattcoach::domain::wahoo::ManualWahooSyncResult {
                scanned: 0,
                imported: 0,
                skipped: 0,
            })
        })
    }
}

#[derive(Clone)]
pub(crate) struct TestIdentityService {
    pub(crate) admin_cookie_role: Role,
    pub(crate) callback_error: Option<IdentityError>,
    pub(crate) current_user_error: Option<IdentityError>,
    pub(crate) join_whitelist_error: Option<IdentityError>,
    pub(crate) last_join_whitelist_email: Arc<Mutex<Option<String>>>,
    pub(crate) last_callback_input: Arc<Mutex<Option<(String, String)>>>,
    pub(crate) pending_approval_redirect_to: Option<String>,
    pub(crate) last_logout_session_id: Arc<Mutex<Option<String>>>,
    pub(crate) last_return_to: Arc<Mutex<Option<String>>>,
    pub(crate) logout_error: Option<IdentityError>,
    pub(crate) require_admin_error: Option<IdentityError>,
}

impl Default for TestIdentityService {
    fn default() -> Self {
        Self {
            admin_cookie_role: Role::Admin,
            callback_error: None,
            current_user_error: None,
            join_whitelist_error: None,
            last_join_whitelist_email: Arc::new(Mutex::new(None)),
            last_callback_input: Arc::new(Mutex::new(None)),
            pending_approval_redirect_to: None,
            last_logout_session_id: Arc::new(Mutex::new(None)),
            last_return_to: Arc::new(Mutex::new(None)),
            logout_error: None,
            require_admin_error: None,
        }
    }
}

impl IdentityUseCases for TestIdentityService {
    fn begin_google_login(
        &self,
        return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
        *self.last_return_to.lock().unwrap() = return_to;
        Box::pin(async {
            Ok(GoogleLoginStart {
                state: "state-1".to_string(),
                redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
                    .to_string(),
            })
        })
    }

    fn join_whitelist(&self, email: String) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
        *self.last_join_whitelist_email.lock().unwrap() = Some(email.clone());
        if let Some(error) = self.join_whitelist_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        Box::pin(async move { Ok(WhitelistEntry::new(email, false, 100, 100)) })
    }

    fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>> {
        *self.last_callback_input.lock().unwrap() = Some((state.to_string(), code.to_string()));
        if let Some(error) = self.callback_error.clone() {
            return Box::pin(async move { Err(error) });
        }
        if let Some(redirect_to) = self.pending_approval_redirect_to.clone() {
            return Box::pin(
                async move { Ok(GoogleLoginOutcome::PendingApproval { redirect_to }) },
            );
        }

        let role = self.admin_cookie_role.clone();
        Box::pin(async move {
            Ok(GoogleLoginOutcome::SignedIn(Box::new(GoogleLoginSuccess {
                user: AppUser::new(
                    "user-1".to_string(),
                    "google-subject-1".to_string(),
                    "admin@example.com".to_string(),
                    vec![Role::User, role.clone()],
                    Some("Admin Athlete".to_string()),
                    Some("https://example.com/avatar.png".to_string()),
                    true,
                ),
                session: AuthSession::new(
                    "session-1".to_string(),
                    "user-1".to_string(),
                    999999,
                    100,
                ),
                redirect_to: "/calendar".to_string(),
            })))
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        if let Some(error) = self.current_user_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        let role = self.admin_cookie_role.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            if session_id != "session-1" {
                return Ok(None);
            }

            let mut roles = vec![Role::User];
            if role == Role::Admin {
                roles.push(Role::Admin);
            }

            Ok(Some(AppUser::new(
                "user-1".to_string(),
                "google-subject-1".to_string(),
                "admin@example.com".to_string(),
                roles,
                Some("Admin Athlete".to_string()),
                Some("https://example.com/avatar.png".to_string()),
                true,
            )))
        })
    }

    fn logout(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        *self.last_logout_session_id.lock().unwrap() = Some(session_id.to_string());
        if let Some(error) = self.logout_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        if let Some(error) = self.require_admin_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        let role = self.admin_cookie_role.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            if session_id != "session-1" {
                return Err(IdentityError::Unauthenticated);
            }

            if role != Role::Admin {
                return Err(IdentityError::Forbidden);
            }

            Ok(AppUser::new(
                "user-1".to_string(),
                "google-subject-1".to_string(),
                "admin@example.com".to_string(),
                vec![Role::User, Role::Admin],
                Some("Admin Athlete".to_string()),
                Some("https://example.com/avatar.png".to_string()),
                true,
            ))
        })
    }
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}

fn shared_frontend_fixture() -> &'static FrontendFixture {
    SHARED_FRONTEND_FIXTURE.get_or_init(frontend_fixture)
}

fn frontend_fixture() -> FrontendFixture {
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-auth-spa-fixture-{}",
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

struct FrontendFixture {
    root: PathBuf,
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
