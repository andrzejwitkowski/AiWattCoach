use super::{
    assign_roles, authorize_admin_access, AppUser, AuthSession, BoxFuture, Clock, GoogleOAuthPort,
    IdGenerator, IdentityError, LoginState, LoginStateRepository, SessionRepository,
    UserRepository,
};

pub trait IdentityUseCases: Send + Sync {
    fn begin_google_login(
        &self,
        return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>>;
    fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<GoogleLoginSuccess, IdentityError>>;
    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>>;
    fn logout(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>>;
    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>>;
}

fn sanitize_return_to(raw_return_to: Option<String>) -> Option<String> {
    raw_return_to.and_then(|value| {
        let trimmed = value.trim();
        let lower = trimmed.to_ascii_lowercase();

        if trimmed.is_empty()
            || !trimmed.starts_with('/')
            || trimmed.starts_with("//")
            || trimmed.contains(':')
            || trimmed.contains('\\')
            || trimmed.chars().any(|character| character.is_control())
            || lower.contains("%0d")
            || lower.contains("%0a")
        {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogleLoginStart {
    pub state: String,
    pub redirect_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogleLoginSuccess {
    pub user: AppUser,
    pub session: AuthSession,
    pub redirect_to: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityServiceConfig {
    pub admin_emails: Vec<String>,
    pub session_ttl_hours: u64,
}

impl IdentityServiceConfig {
    pub fn new(admin_emails: Vec<String>, session_ttl_hours: u64) -> Self {
        Self {
            admin_emails,
            session_ttl_hours,
        }
    }
}

#[derive(Clone)]
pub struct IdentityService<Users, Sessions, LoginStates, GoogleOAuth, Time, Ids>
where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    GoogleOAuth: GoogleOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    users: Users,
    sessions: Sessions,
    login_states: LoginStates,
    google_oauth: GoogleOAuth,
    clock: Time,
    ids: Ids,
    admin_emails: Vec<String>,
    session_ttl_hours: u64,
}

impl<Users, Sessions, LoginStates, GoogleOAuth, Time, Ids>
    IdentityService<Users, Sessions, LoginStates, GoogleOAuth, Time, Ids>
where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    GoogleOAuth: GoogleOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        users: Users,
        sessions: Sessions,
        login_states: LoginStates,
        google_oauth: GoogleOAuth,
        clock: Time,
        ids: Ids,
        config: IdentityServiceConfig,
    ) -> Self {
        Self {
            users,
            sessions,
            login_states,
            google_oauth,
            clock,
            ids,
            admin_emails: config.admin_emails,
            session_ttl_hours: config.session_ttl_hours,
        }
    }

    async fn get_valid_session(
        &self,
        session_id: &str,
    ) -> Result<Option<AuthSession>, IdentityError> {
        let now = self.clock.now_epoch_seconds();
        let session = self.sessions.find_by_id(session_id).await?;

        if let Some(session) = session {
            if session.is_expired(now) {
                self.sessions.delete(session_id).await?;
                return Ok(None);
            }

            return Ok(Some(session));
        }

        Ok(None)
    }

    pub async fn begin_google_login(
        &self,
        return_to: Option<String>,
    ) -> Result<GoogleLoginStart, IdentityError> {
        let now = self.clock.now_epoch_seconds();
        let state = self.ids.new_id("login-state");
        let login_state =
            LoginState::new(state.clone(), sanitize_return_to(return_to), now + 600, now);

        self.login_states.create(login_state).await?;
        let redirect_url = self.google_oauth.build_authorize_url(&state)?;

        Ok(GoogleLoginStart {
            state,
            redirect_url,
        })
    }

    pub async fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> Result<GoogleLoginSuccess, IdentityError> {
        let now = self.clock.now_epoch_seconds();
        let login_state = self
            .login_states
            .consume(state)
            .await?
            .filter(|saved_state| !saved_state.is_expired(now))
            .ok_or(IdentityError::InvalidLoginState)?;

        let google_identity = self.google_oauth.exchange_code_for_identity(code).await?;

        let roles = assign_roles(&google_identity.email, &self.admin_emails);
        let user = self
            .users
            .save_google_user_for_identity(self.ids.new_id("user"), google_identity, roles)
            .await?;

        let session = self
            .sessions
            .save(AuthSession::new(
                self.ids.new_id("session"),
                user.id.clone(),
                compute_session_expiry(now, self.session_ttl_hours)?,
                now,
            ))
            .await?;

        Ok(GoogleLoginSuccess {
            user,
            session,
            redirect_to: sanitize_return_to(login_state.return_to)
                .unwrap_or_else(|| "/app".to_string()),
        })
    }

    pub async fn logout(&self, session_id: &str) -> Result<(), IdentityError> {
        self.sessions.delete(session_id).await
    }

    pub async fn get_current_user(
        &self,
        session_id: &str,
    ) -> Result<Option<AppUser>, IdentityError> {
        let Some(session) = self.get_valid_session(session_id).await? else {
            return Ok(None);
        };

        self.users.find_by_id(&session.user_id).await
    }

    pub async fn require_admin(&self, session_id: &str) -> Result<AppUser, IdentityError> {
        let user = self
            .get_current_user(session_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        authorize_admin_access(&user)?;
        Ok(user)
    }
}

fn compute_session_expiry(
    now_epoch_seconds: i64,
    session_ttl_hours: u64,
) -> Result<i64, IdentityError> {
    const MAX_BSON_EPOCH_SECONDS: i64 = i64::MAX / 1000;

    let ttl_hours = i64::try_from(session_ttl_hours).map_err(|_| {
        IdentityError::External("SESSION_TTL_HOURS exceeds supported range".to_string())
    })?;
    let ttl_seconds = ttl_hours.checked_mul(3600).ok_or_else(|| {
        IdentityError::External("SESSION_TTL_HOURS exceeds supported range".to_string())
    })?;

    let expires_at = now_epoch_seconds.checked_add(ttl_seconds).ok_or_else(|| {
        IdentityError::External("SESSION_TTL_HOURS exceeds supported range".to_string())
    })?;

    if expires_at > MAX_BSON_EPOCH_SECONDS {
        return Err(IdentityError::External(
            "SESSION_TTL_HOURS exceeds supported range".to_string(),
        ));
    }

    Ok(expires_at)
}

impl<Users, Sessions, LoginStates, GoogleOAuth, Time, Ids> IdentityUseCases
    for IdentityService<Users, Sessions, LoginStates, GoogleOAuth, Time, Ids>
where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    GoogleOAuth: GoogleOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    fn begin_google_login(
        &self,
        return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
        let service = self.clone();
        Box::pin(async move { service.begin_google_login(return_to).await })
    }

    fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<GoogleLoginSuccess, IdentityError>> {
        let service = self.clone();
        let state = state.to_string();
        let code = code.to_string();
        Box::pin(async move { service.handle_google_callback(&state, &code).await })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let service = self.clone();
        let session_id = session_id.to_string();
        Box::pin(async move { service.get_current_user(&session_id).await })
    }

    fn logout(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let service = self.clone();
        let session_id = session_id.to_string();
        Box::pin(async move { service.logout(&session_id).await })
    }

    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        let service = self.clone();
        let session_id = session_id.to_string();
        Box::pin(async move { service.require_admin(&session_id).await })
    }
}
