use crate::domain::{
    identity::{Clock, IdGenerator},
    settings::{UserSettings, UserSettingsRepository, WahooConfig},
};

use super::{
    BoxFuture, WahooAuthExchange, WahooAuthStart, WahooConnectState, WahooConnectStateRepository,
    WahooError, WahooOAuthPort, WahooToken,
};

const CONNECT_STATE_TTL_SECONDS: i64 = 600;

pub trait WahooUseCases: Send + Sync {
    fn begin_connect(
        &self,
        user_id: &str,
        return_to: Option<String>,
    ) -> BoxFuture<Result<WahooAuthStart, WahooError>>;

    fn finish_connect(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<WahooAuthExchange, WahooError>>;

    fn ensure_token(&self, user_id: &str) -> BoxFuture<Result<WahooToken, WahooError>>;
}

#[derive(Clone)]
pub struct WahooService<SettingsRepo, ConnectStates, OAuth, Time, Ids>
where
    SettingsRepo: UserSettingsRepository,
    ConnectStates: WahooConnectStateRepository,
    OAuth: WahooOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    settings_repository: SettingsRepo,
    connect_states: ConnectStates,
    oauth: OAuth,
    clock: Time,
    ids: Ids,
}

impl<SettingsRepo, ConnectStates, OAuth, Time, Ids>
    WahooService<SettingsRepo, ConnectStates, OAuth, Time, Ids>
where
    SettingsRepo: UserSettingsRepository,
    ConnectStates: WahooConnectStateRepository,
    OAuth: WahooOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        settings_repository: SettingsRepo,
        connect_states: ConnectStates,
        oauth: OAuth,
        clock: Time,
        ids: Ids,
    ) -> Self {
        Self {
            settings_repository,
            connect_states,
            oauth,
            clock,
            ids,
        }
    }

    async fn get_or_create_settings(&self, user_id: &str) -> Result<UserSettings, WahooError> {
        if let Some(settings) = self
            .settings_repository
            .find_by_user_id(user_id)
            .await
            .map_err(map_settings_error)?
        {
            return Ok(settings);
        }

        let now = self.clock.now_epoch_seconds();
        self.settings_repository
            .upsert(UserSettings::new_defaults(user_id.to_string(), now))
            .await
            .map_err(map_settings_error)
    }

    async fn persist_token(
        &self,
        user_id: &str,
        token: WahooToken,
    ) -> Result<WahooToken, WahooError> {
        let mut settings = self.get_or_create_settings(user_id).await?;
        settings.wahoo = WahooConfig {
            access_token: Some(token.access_token.clone()),
            refresh_token: Some(token.refresh_token.clone()),
            expires_at_epoch_seconds: Some(token.expires_at_epoch_seconds),
            connected: true,
        };
        settings.updated_at_epoch_seconds = self.clock.now_epoch_seconds();
        self.settings_repository
            .upsert(settings)
            .await
            .map_err(map_settings_error)?;
        Ok(token)
    }

    async fn begin_connect(
        &self,
        user_id: &str,
        return_to: Option<String>,
    ) -> Result<WahooAuthStart, WahooError> {
        self.get_or_create_settings(user_id).await?;
        let now = self.clock.now_epoch_seconds();
        let state = self.ids.new_id("wahoo-connect-state");
        self.connect_states
            .create(WahooConnectState::new(
                state.clone(),
                user_id.to_string(),
                sanitize_return_to(return_to),
                now + CONNECT_STATE_TTL_SECONDS,
                now,
            ))
            .await?;
        let redirect_url = self.oauth.build_authorize_url(&state)?;
        Ok(WahooAuthStart {
            state,
            redirect_url,
        })
    }

    async fn finish_connect(
        &self,
        state: &str,
        code: &str,
    ) -> Result<WahooAuthExchange, WahooError> {
        let now = self.clock.now_epoch_seconds();
        let state = self
            .connect_states
            .consume(state)
            .await?
            .filter(|saved| !saved.is_expired(now))
            .ok_or(WahooError::InvalidConnectState)?;
        let token = self.oauth.exchange_code(code).await?;
        let token = self.persist_token(&state.user_id, token).await?;

        Ok(WahooAuthExchange {
            redirect_to: sanitize_return_to(state.return_to)
                .unwrap_or_else(|| "/settings".to_string()),
            token,
        })
    }

    async fn ensure_token(&self, user_id: &str) -> Result<WahooToken, WahooError> {
        let settings = self.get_or_create_settings(user_id).await?;
        let now = self.clock.now_epoch_seconds();
        let wahoo = settings.wahoo;

        if let (Some(access_token), Some(refresh_token), Some(expires_at_epoch_seconds)) = (
            wahoo.access_token,
            wahoo.refresh_token.clone(),
            wahoo.expires_at_epoch_seconds,
        ) {
            if expires_at_epoch_seconds > now {
                return Ok(WahooToken {
                    access_token,
                    refresh_token,
                    expires_at_epoch_seconds,
                });
            }
        }

        let refresh_token = wahoo.refresh_token.ok_or(WahooError::NotConfigured)?;
        let token = self.oauth.refresh_token(&refresh_token).await?;
        self.persist_token(user_id, token).await
    }
}

impl<SettingsRepo, ConnectStates, OAuth, Time, Ids> WahooUseCases
    for WahooService<SettingsRepo, ConnectStates, OAuth, Time, Ids>
where
    SettingsRepo: UserSettingsRepository,
    ConnectStates: WahooConnectStateRepository,
    OAuth: WahooOAuthPort,
    Time: Clock,
    Ids: IdGenerator,
{
    fn begin_connect(
        &self,
        user_id: &str,
        return_to: Option<String>,
    ) -> BoxFuture<Result<WahooAuthStart, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.begin_connect(&user_id, return_to).await })
    }

    fn finish_connect(
        &self,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<WahooAuthExchange, WahooError>> {
        let service = self.clone();
        let state = state.to_string();
        let code = code.to_string();
        Box::pin(async move { service.finish_connect(&state, &code).await })
    }

    fn ensure_token(&self, user_id: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.ensure_token(&user_id).await })
    }
}

fn map_settings_error(error: crate::domain::settings::SettingsError) -> WahooError {
    WahooError::Repository(error.to_string())
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
