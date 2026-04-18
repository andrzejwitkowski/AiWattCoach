use std::{
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::{AppState, WhitelistRateLimiter},
    domain::identity::{
        AppUser, AuthSession, GoogleLoginOutcome, GoogleLoginStart, GoogleLoginSuccess,
        IdentityError, IdentityUseCases, Role, WhitelistEntry,
    },
    domain::settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsUseCases,
    },
    Settings,
};
use mongodb::Client;

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static FRONTEND_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
static KEPT_FRONTEND_FIXTURES: OnceLock<Mutex<Vec<FrontendFixture>>> = OnceLock::new();

pub(crate) async fn auth_test_app(identity_service: TestIdentityService) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);

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
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);

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
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);

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
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);

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
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);

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

pub(crate) async fn auth_test_app_with_limited_whitelist_rate(
    identity_service: TestIdentityService,
    max_attempts: usize,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);

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

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FRONTEND_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-auth-spa-fixture-{}-{unique}-{counter}",
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

fn keep_frontend_fixture(fixture: FrontendFixture) {
    KEPT_FRONTEND_FIXTURES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(fixture);
}
