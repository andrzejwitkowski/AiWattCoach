use super::{
    assign_roles, authorize_admin_access, is_valid_email, normalize_email, AppUser, AuthSession,
    BoxFuture, Clock, GoogleOAuthPort, IdGenerator, IdentityError, LoginState,
    LoginStateRepository, SessionRepository, UserRepository, WhitelistEntry, WhitelistRepository,
};

pub trait IdentityUseCases: Send + Sync {
    fn begin_google_login(
        &self,
        return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>>;
    fn join_whitelist(&self, email: String) -> BoxFuture<Result<WhitelistEntry, IdentityError>>;
    fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>>;
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
pub enum GoogleLoginOutcome {
    SignedIn(Box<GoogleLoginSuccess>),
    PendingApproval { redirect_to: String },
}

fn build_pending_approval_redirect(return_to: Option<String>) -> String {
    let Some(return_to) = sanitize_return_to(return_to) else {
        return "/?auth=pending-approval".to_string();
    };

    format!(
        "/?auth=pending-approval&returnTo={}",
        urlencoding::encode(&return_to)
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityServiceConfig {
    pub admin_emails: Vec<String>,
    pub session_ttl_hours: u64,
}

pub struct IdentityServiceDependencies<
    Users,
    Sessions,
    LoginStates,
    Whitelist,
    GoogleOAuth,
    Time,
    Ids,
> where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    Whitelist: WhitelistRepository,
    GoogleOAuth: GoogleOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    pub users: Users,
    pub sessions: Sessions,
    pub login_states: LoginStates,
    pub whitelist: Whitelist,
    pub google_oauth: GoogleOAuth,
    pub clock: Time,
    pub ids: Ids,
}

pub const MAX_BSON_EPOCH_SECONDS: i64 = i64::MAX / 1000;

impl IdentityServiceConfig {
    pub fn new(admin_emails: Vec<String>, session_ttl_hours: u64) -> Self {
        Self {
            admin_emails,
            session_ttl_hours,
        }
    }
}

#[derive(Clone)]
pub struct IdentityService<Users, Sessions, LoginStates, Whitelist, GoogleOAuth, Time, Ids>
where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    Whitelist: WhitelistRepository,
    GoogleOAuth: GoogleOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    users: Users,
    sessions: Sessions,
    login_states: LoginStates,
    whitelist: Whitelist,
    google_oauth: GoogleOAuth,
    clock: Time,
    ids: Ids,
    admin_emails: Vec<String>,
    session_ttl_hours: u64,
}

impl<Users, Sessions, LoginStates, Whitelist, GoogleOAuth, Time, Ids>
    IdentityService<Users, Sessions, LoginStates, Whitelist, GoogleOAuth, Time, Ids>
where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    Whitelist: WhitelistRepository,
    GoogleOAuth: GoogleOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        dependencies: IdentityServiceDependencies<
            Users,
            Sessions,
            LoginStates,
            Whitelist,
            GoogleOAuth,
            Time,
            Ids,
        >,
        config: IdentityServiceConfig,
    ) -> Self {
        Self {
            users: dependencies.users,
            sessions: dependencies.sessions,
            login_states: dependencies.login_states,
            whitelist: dependencies.whitelist,
            google_oauth: dependencies.google_oauth,
            clock: dependencies.clock,
            ids: dependencies.ids,
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

    pub async fn join_whitelist(&self, email: String) -> Result<WhitelistEntry, IdentityError> {
        if !is_valid_email(&email) {
            return Err(IdentityError::InvalidEmail);
        }

        let now = self.clock.now_epoch_seconds();
        let normalized_email = normalize_email(&email);

        if let Some(existing) = self
            .whitelist
            .find_by_normalized_email(&normalized_email)
            .await?
        {
            return self
                .whitelist
                .save(WhitelistEntry::new(
                    existing.email,
                    existing.allowed,
                    existing.created_at_epoch_seconds,
                    now,
                ))
                .await;
        }

        self.whitelist
            .save(WhitelistEntry::new(email, false, now, now))
            .await
    }

    pub async fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> Result<GoogleLoginOutcome, IdentityError> {
        let now = self.clock.now_epoch_seconds();
        let login_state = self
            .login_states
            .consume(state)
            .await?
            .filter(|saved_state| !saved_state.is_expired(now))
            .ok_or(IdentityError::InvalidLoginState)?;

        let google_identity = self.google_oauth.exchange_code_for_identity(code).await?;

        let existing_user = match self
            .users
            .find_by_google_subject(&google_identity.subject)
            .await?
        {
            Some(user) => Some(user),
            None => {
                self.users
                    .find_by_normalized_email(&google_identity.email_normalized)
                    .await?
            }
        };

        if existing_user.is_none() {
            let redirect_to = build_pending_approval_redirect(login_state.return_to.clone());
            let whitelist_entry = self
                .whitelist
                .find_by_normalized_email(&google_identity.email_normalized)
                .await?;

            match whitelist_entry {
                Some(entry) if entry.allowed => {
                    // Allowed whitelisted first-time user: proceed to account creation below.
                    let _ = entry;
                }
                Some(entry) => {
                    self.whitelist
                        .save(WhitelistEntry::new(
                            entry.email,
                            false,
                            entry.created_at_epoch_seconds,
                            now,
                        ))
                        .await?;
                    return Ok(GoogleLoginOutcome::PendingApproval { redirect_to });
                }
                None => {
                    self.whitelist
                        .save(WhitelistEntry::new(
                            google_identity.email.clone(),
                            false,
                            now,
                            now,
                        ))
                        .await?;
                    return Ok(GoogleLoginOutcome::PendingApproval { redirect_to });
                }
            }
        }

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

        Ok(GoogleLoginOutcome::SignedIn(Box::new(GoogleLoginSuccess {
            user,
            session,
            redirect_to: sanitize_return_to(login_state.return_to)
                .unwrap_or_else(|| "/calendar".to_string()),
        })))
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

pub fn validate_session_ttl_against_current_time(
    now_epoch_seconds: i64,
    session_ttl_hours: u64,
) -> Result<(), IdentityError> {
    compute_session_expiry(now_epoch_seconds, session_ttl_hours).map(|_| ())
}

impl<Users, Sessions, LoginStates, Whitelist, GoogleOAuth, Time, Ids> IdentityUseCases
    for IdentityService<Users, Sessions, LoginStates, Whitelist, GoogleOAuth, Time, Ids>
where
    Users: UserRepository,
    Sessions: SessionRepository,
    LoginStates: LoginStateRepository,
    Whitelist: WhitelistRepository,
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

    fn join_whitelist(&self, email: String) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
        let service = self.clone();
        Box::pin(async move { service.join_whitelist(email).await })
    }

    fn handle_google_callback(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>> {
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
