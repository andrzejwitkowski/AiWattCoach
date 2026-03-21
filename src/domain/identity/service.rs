use super::{
    assign_roles, authorize_admin_access, AppUser, AuthSession, BoxFuture, Clock,
    GoogleOAuthPort, IdGenerator, IdentityError, LoginState, LoginStateRepository,
    SessionRepository, UserRepository,
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

        if trimmed.is_empty()
            || !trimmed.starts_with('/')
            || trimmed.starts_with("//")
            || trimmed.contains(':')
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
    pub sessions: Sessions,
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
        admin_emails: Vec<String>,
        session_ttl_hours: u64,
    ) -> Self {
        Self {
            users,
            sessions,
            login_states,
            google_oauth,
            clock,
            ids,
            admin_emails,
            session_ttl_hours,
        }
    }

    async fn get_valid_session(&self, session_id: &str) -> Result<Option<AuthSession>, IdentityError> {
        let now = self.clock.now_epoch_seconds();
        let session = self.sessions.find_by_id(session_id).await?;

        if let Some(session) = session {
            if session.expires_at_epoch_seconds < now {
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
        let login_state = LoginState::new(state.clone(), sanitize_return_to(return_to), now + 600, now);

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
            .find_by_id(state)
            .await?
            .filter(|saved_state| !saved_state.is_expired(now))
            .ok_or(IdentityError::InvalidLoginState)?;

        self.login_states.delete(state).await?;

        let google_identity = self.google_oauth.exchange_code_for_identity(code).await?;

        let roles = assign_roles(&google_identity.email, &self.admin_emails);
        let user = if let Some(existing_user) = self
            .users
            .find_by_google_subject(&google_identity.subject)
            .await?
        {
            self.users
                .save(AppUser::new(
                    existing_user.id,
                    google_identity.subject.clone(),
                    google_identity.email.clone(),
                    roles,
                    google_identity.display_name.clone(),
                    google_identity.avatar_url.clone(),
                    google_identity.email_verified,
                ))
                .await?
        } else if let Some(existing_user) = self
            .users
            .find_by_normalized_email(&google_identity.email_normalized)
            .await?
        {
            self.users
                .save(AppUser::new(
                    existing_user.id,
                    google_identity.subject.clone(),
                    google_identity.email.clone(),
                    roles,
                    google_identity.display_name.clone(),
                    google_identity.avatar_url.clone(),
                    google_identity.email_verified,
                ))
                .await?
        } else {
            self.users
                .save(AppUser::new(
                    self.ids.new_id("user"),
                    google_identity.subject.clone(),
                    google_identity.email.clone(),
                    roles,
                    google_identity.display_name.clone(),
                    google_identity.avatar_url.clone(),
                    google_identity.email_verified,
                ))
                .await?
        };

        let session = self
            .sessions
            .save(AuthSession::new(
                self.ids.new_id("session"),
                user.id.clone(),
                now + (self.session_ttl_hours as i64 * 3600),
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

    pub async fn get_current_user(&self, session_id: &str) -> Result<Option<AppUser>, IdentityError> {
        let Some(session) = self.get_valid_session(session_id).await? else {
            return Ok(None);
        };

        self.users.find_by_id(&session.user_id).await
    }

    pub async fn require_admin(&self, session_id: &str) -> Result<AppUser, IdentityError> {
        let user = self
            .get_current_user(session_id)
            .await?
            .ok_or(IdentityError::Forbidden)?;
        authorize_admin_access(&user)?;
        Ok(user)
    }
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
