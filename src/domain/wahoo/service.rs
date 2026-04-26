use crate::domain::{
    identity::{Clock, IdGenerator},
    return_to::sanitize_return_to,
    settings::{UserSettings, UserSettingsRepository, WahooConfig},
};
use std::sync::Arc;

use super::{
    BoxFuture, WahooApiPort, WahooAuthExchange, WahooAuthStart, WahooConnectState,
    WahooConnectStateRepository, WahooCreatePlan, WahooCreateWorkout, WahooError, WahooOAuthPort,
    WahooPlan, WahooToken, WahooUpdatePlan, WahooUpdateWorkout, WahooWorkout, WahooWorkoutList,
    WahooWorkoutSummary,
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
        user_id: &str,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<WahooAuthExchange, WahooError>>;

    fn ensure_token(&self, user_id: &str) -> BoxFuture<Result<WahooToken, WahooError>>;

    fn list_workouts(
        &self,
        user_id: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>>;

    fn get_workout(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>>;

    fn get_workout_summary(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>>;

    fn find_plan_by_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<WahooPlan>, WahooError>>;

    fn create_plan(
        &self,
        user_id: &str,
        request: WahooCreatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>>;

    fn update_plan(
        &self,
        user_id: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>>;

    fn create_workout(
        &self,
        user_id: &str,
        request: WahooCreateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>>;

    fn update_workout(
        &self,
        user_id: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>>;

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>>;
}

impl<T> WahooUseCases for Arc<T>
where
    T: WahooUseCases + ?Sized,
{
    fn begin_connect(
        &self,
        user_id: &str,
        return_to: Option<String>,
    ) -> BoxFuture<Result<WahooAuthStart, WahooError>> {
        self.as_ref().begin_connect(user_id, return_to)
    }

    fn finish_connect(
        &self,
        user_id: &str,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<WahooAuthExchange, WahooError>> {
        self.as_ref().finish_connect(user_id, state, code)
    }

    fn ensure_token(&self, user_id: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        self.as_ref().ensure_token(user_id)
    }

    fn list_workouts(
        &self,
        user_id: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>> {
        self.as_ref().list_workouts(user_id, page, per_page)
    }

    fn get_workout(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        self.as_ref().get_workout(user_id, workout_id)
    }

    fn get_workout_summary(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        self.as_ref().get_workout_summary(user_id, workout_id)
    }

    fn find_plan_by_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<WahooPlan>, WahooError>> {
        self.as_ref().find_plan_by_external_id(user_id, external_id)
    }

    fn create_plan(
        &self,
        user_id: &str,
        request: WahooCreatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        self.as_ref().create_plan(user_id, request)
    }

    fn update_plan(
        &self,
        user_id: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        self.as_ref().update_plan(user_id, plan_id, request)
    }

    fn create_workout(
        &self,
        user_id: &str,
        request: WahooCreateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        self.as_ref().create_workout(user_id, request)
    }

    fn update_workout(
        &self,
        user_id: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        self.as_ref().update_workout(user_id, workout_id, request)
    }

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>> {
        self.as_ref().download_workout_file(file_url)
    }
}

#[derive(Clone)]
pub struct WahooService<SettingsRepo, ConnectStates, Client, Time, Ids>
where
    SettingsRepo: UserSettingsRepository,
    ConnectStates: WahooConnectStateRepository,
    Client: WahooOAuthPort + WahooApiPort,
    Time: Clock,
    Ids: IdGenerator,
{
    settings_repository: SettingsRepo,
    connect_states: ConnectStates,
    client: Client,
    clock: Time,
    ids: Ids,
}

impl<SettingsRepo, ConnectStates, Client, Time, Ids>
    WahooService<SettingsRepo, ConnectStates, Client, Time, Ids>
where
    SettingsRepo: UserSettingsRepository,
    ConnectStates: WahooConnectStateRepository,
    Client: WahooOAuthPort + WahooApiPort,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        settings_repository: SettingsRepo,
        connect_states: ConnectStates,
        client: Client,
        clock: Time,
        ids: Ids,
    ) -> Self {
        Self {
            settings_repository,
            connect_states,
            client,
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
        mark_connection_updated: bool,
    ) -> Result<WahooToken, WahooError> {
        let mut settings = self.get_or_create_settings(user_id).await?;
        let connection_updated_at_epoch_seconds = if mark_connection_updated {
            Some(self.clock.now_epoch_seconds())
        } else {
            settings.wahoo.updated_at_epoch_seconds
        };
        settings.wahoo = WahooConfig {
            access_token: Some(token.access_token.clone()),
            refresh_token: Some(token.refresh_token.clone()),
            expires_at_epoch_seconds: Some(token.expires_at_epoch_seconds),
            connected: true,
            updated_at_epoch_seconds: connection_updated_at_epoch_seconds,
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
        let redirect_url = self.client.build_authorize_url(&state)?;
        Ok(WahooAuthStart {
            state,
            redirect_url,
        })
    }

    async fn finish_connect(
        &self,
        user_id: &str,
        state: &str,
        code: &str,
    ) -> Result<WahooAuthExchange, WahooError> {
        let now = self.clock.now_epoch_seconds();
        let state = self
            .connect_states
            .consume(state, user_id)
            .await?
            .filter(|saved| !saved.is_expired(now))
            .ok_or(WahooError::InvalidConnectState)?;
        let token = self.client.exchange_code(code).await?;
        let token = self.persist_token(&state.user_id, token, true).await?;

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

        let refresh_token = wahoo.refresh_token.ok_or(WahooError::NotConnected)?;
        let token = self.client.refresh_token(&refresh_token).await?;
        self.persist_token(user_id, token, false).await
    }

    async fn list_workouts(
        &self,
        user_id: &str,
        page: usize,
        per_page: usize,
    ) -> Result<WahooWorkoutList, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client
            .list_workouts(&token.access_token, page, per_page)
            .await
    }

    async fn get_workout(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> Result<WahooWorkout, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client
            .get_workout(&token.access_token, workout_id)
            .await
    }

    async fn get_workout_summary(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> Result<Option<WahooWorkoutSummary>, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client
            .get_workout_summary(&token.access_token, workout_id)
            .await
    }

    async fn find_plan_by_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> Result<Option<WahooPlan>, WahooError> {
        let token = self.ensure_token(user_id).await?;
        let plans = self
            .client
            .list_plans(&token.access_token, Some(external_id))
            .await?;

        match plans.len() {
            0 => Ok(None),
            1 => Ok(plans.into_iter().next()),
            count => Err(WahooError::External(format!(
                "Wahoo returned {count} plans for external_id '{external_id}'"
            ))),
        }
    }

    async fn create_plan(
        &self,
        user_id: &str,
        request: WahooCreatePlan,
    ) -> Result<WahooPlan, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client.create_plan(&token.access_token, request).await
    }

    async fn update_plan(
        &self,
        user_id: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> Result<WahooPlan, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client
            .update_plan(&token.access_token, plan_id, request)
            .await
    }

    async fn create_workout(
        &self,
        user_id: &str,
        request: WahooCreateWorkout,
    ) -> Result<WahooWorkout, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client
            .create_workout(&token.access_token, request)
            .await
    }

    async fn update_workout(
        &self,
        user_id: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> Result<WahooWorkout, WahooError> {
        let token = self.ensure_token(user_id).await?;
        self.client
            .update_workout(&token.access_token, workout_id, request)
            .await
    }

    async fn download_workout_file(&self, file_url: &str) -> Result<Vec<u8>, WahooError> {
        self.client.download_workout_file(file_url).await
    }
}

impl<SettingsRepo, ConnectStates, Client, Time, Ids> WahooUseCases
    for WahooService<SettingsRepo, ConnectStates, Client, Time, Ids>
where
    SettingsRepo: UserSettingsRepository,
    ConnectStates: WahooConnectStateRepository,
    Client: WahooOAuthPort + WahooApiPort,
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
        user_id: &str,
        state: &str,
        code: &str,
    ) -> BoxFuture<Result<WahooAuthExchange, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let state = state.to_string();
        let code = code.to_string();
        Box::pin(async move { service.finish_connect(&user_id, &state, &code).await })
    }

    fn ensure_token(&self, user_id: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.ensure_token(&user_id).await })
    }

    fn list_workouts(
        &self,
        user_id: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.list_workouts(&user_id, page, per_page).await })
    }

    fn get_workout(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_workout(&user_id, workout_id).await })
    }

    fn get_workout_summary(
        &self,
        user_id: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_workout_summary(&user_id, workout_id).await })
    }

    fn find_plan_by_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<WahooPlan>, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            service
                .find_plan_by_external_id(&user_id, &external_id)
                .await
        })
    }

    fn create_plan(
        &self,
        user_id: &str,
        request: WahooCreatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.create_plan(&user_id, request).await })
    }

    fn update_plan(
        &self,
        user_id: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.update_plan(&user_id, plan_id, request).await })
    }

    fn create_workout(
        &self,
        user_id: &str,
        request: WahooCreateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.create_workout(&user_id, request).await })
    }

    fn update_workout(
        &self,
        user_id: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.update_workout(&user_id, workout_id, request).await })
    }

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>> {
        let service = self.clone();
        let file_url = file_url.to_string();
        Box::pin(async move { service.download_workout_file(&file_url).await })
    }
}

fn map_settings_error(error: crate::domain::settings::SettingsError) -> WahooError {
    WahooError::Repository(error.to_string())
}
