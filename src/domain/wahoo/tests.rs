use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use super::{
    WahooApiPort, WahooConnectState, WahooConnectStateRepository, WahooCreatePlan,
    WahooCreateWorkout, WahooError, WahooOAuthPort, WahooPlan, WahooService, WahooToken,
    WahooUpdatePlan, WahooUpdateWorkout, WahooUseCases, WahooWorkout, WahooWorkoutList,
    WahooWorkoutSummary,
};
use crate::domain::{
    identity::{Clock, IdGenerator},
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsRepository,
    },
};

#[derive(Clone)]
struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        100
    }
}

#[derive(Clone)]
struct TestIds;

impl IdGenerator for TestIds {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-1")
    }
}

#[derive(Clone, Default)]
struct InMemorySettingsRepository {
    items: Arc<Mutex<BTreeMap<String, UserSettings>>>,
}

impl UserSettingsRepository for InMemorySettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let items = self.items.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(items.lock().unwrap().get(&user_id).cloned()) })
    }

    fn upsert(
        &self,
        settings: UserSettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        let items = self.items.clone();
        Box::pin(async move {
            items
                .lock()
                .unwrap()
                .insert(settings.user_id.clone(), settings.clone());
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }
}

#[derive(Clone, Default)]
struct InMemoryConnectStates {
    items: Arc<Mutex<Vec<WahooConnectState>>>,
}

impl WahooConnectStateRepository for InMemoryConnectStates {
    fn create(
        &self,
        state: WahooConnectState,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooConnectState, WahooError>> {
        let items = self.items.clone();
        Box::pin(async move {
            items.lock().unwrap().push(state.clone());
            Ok(state)
        })
    }

    fn consume(
        &self,
        state_id: &str,
        user_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<Option<WahooConnectState>, WahooError>> {
        let items = self.items.clone();
        let state_id = state_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut items = items.lock().unwrap();
            let index = items
                .iter()
                .position(|state| state.id == state_id && state.user_id == user_id);
            Ok(index.map(|position| items.remove(position)))
        })
    }
}

#[derive(Clone, Default)]
struct TestOAuth {
    last_code: Arc<Mutex<Option<String>>>,
    plans: Arc<Mutex<Vec<WahooPlan>>>,
}

impl WahooOAuthPort for TestOAuth {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError> {
        Ok(format!("https://example.com/oauth?state={state}"))
    }

    fn exchange_code(
        &self,
        code: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooToken, WahooError>> {
        *self.last_code.lock().unwrap() = Some(code.to_string());
        Box::pin(async move {
            Ok(WahooToken {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires_at_epoch_seconds: 1_800,
            })
        })
    }

    fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooToken, WahooError>> {
        let refresh_token = refresh_token.to_string();
        Box::pin(async move {
            Ok(WahooToken {
                access_token: "refreshed-access-token".to_string(),
                refresh_token,
                expires_at_epoch_seconds: 1_800,
            })
        })
    }
}

impl WahooApiPort for TestOAuth {
    fn list_plans(
        &self,
        _access_token: &str,
        _external_id: Option<&str>,
    ) -> crate::domain::wahoo::BoxFuture<Result<Vec<WahooPlan>, WahooError>> {
        let plans = self.plans.clone();
        Box::pin(async move { Ok(plans.lock().unwrap().clone()) })
    }

    fn create_plan(
        &self,
        _access_token: &str,
        _request: WahooCreatePlan,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_plan(
        &self,
        _access_token: &str,
        _plan_id: i64,
        _request: WahooUpdatePlan,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn list_workouts(
        &self,
        _access_token: &str,
        _page: usize,
        _per_page: usize,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkoutList, WahooError>> {
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
        _access_token: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotFound) })
    }

    fn get_workout_summary(
        &self,
        _access_token: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn create_workout(
        &self,
        _access_token: &str,
        _request: WahooCreateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_workout(
        &self,
        _access_token: &str,
        _workout_id: i64,
        _request: WahooUpdateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn download_workout_file(
        &self,
        _file_url: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<Vec<u8>, WahooError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[tokio::test]
async fn finish_connect_rejects_state_owned_by_another_user() {
    let settings = InMemorySettingsRepository::default();
    let connect_states = InMemoryConnectStates::default();
    let oauth = TestOAuth::default();
    let service = WahooService::new(
        settings,
        connect_states.clone(),
        oauth.clone(),
        TestClock,
        TestIds,
    );

    connect_states
        .create(WahooConnectState::new(
            "state-1".to_string(),
            "user-1".to_string(),
            Some("/settings?tab=integrations".to_string()),
            200,
            100,
        ))
        .await
        .unwrap();

    let error = service
        .finish_connect("user-2", "state-1", "oauth-code")
        .await
        .unwrap_err();

    assert_eq!(error, WahooError::InvalidConnectState);
    assert_eq!(*oauth.last_code.lock().unwrap(), None);
    assert_eq!(connect_states.items.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn find_plan_by_external_id_rejects_duplicate_external_ids() {
    let settings = InMemorySettingsRepository::default();
    let mut user_settings = UserSettings::new_defaults("user-1".to_string(), 100);
    user_settings.wahoo.connected = true;
    user_settings.wahoo.access_token = Some("access-token".to_string());
    user_settings.wahoo.refresh_token = Some("refresh-token".to_string());
    user_settings.wahoo.expires_at_epoch_seconds = Some(1_000);
    settings.upsert(user_settings).await.unwrap();
    let connect_states = InMemoryConnectStates::default();
    let oauth = TestOAuth::default();
    oauth.plans.lock().unwrap().extend([
        WahooPlan {
            id: 1,
            external_id: "plan-1".to_string(),
            provider_updated_at: None,
            filename: None,
            name: None,
            description: None,
            created_at: None,
            updated_at: None,
        },
        WahooPlan {
            id: 2,
            external_id: "plan-1".to_string(),
            provider_updated_at: None,
            filename: None,
            name: None,
            description: None,
            created_at: None,
            updated_at: None,
        },
    ]);
    let service = WahooService::new(settings, connect_states, oauth, TestClock, TestIds);

    let error = service
        .find_plan_by_external_id("user-1", "plan-1")
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WahooError::External("Wahoo returned 2 plans for external_id 'plan-1'".to_string())
    );
}
