use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use super::{
    WahooApiPort, WahooConnectState, WahooConnectStateRepository, WahooCreatePlan,
    WahooCreateWorkout, WahooError, WahooOAuthPort, WahooPlan, WahooService, WahooToken,
    WahooUpdatePlan, WahooUpdateWorkout, WahooUseCases, WahooUser, WahooWorkout, WahooWorkoutList,
    WahooWorkoutSummary,
};
use crate::domain::{
    identity::{Clock, IdGenerator},
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsRepository, WahooUserIdBackfillCandidate,
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

    fn find_by_wahoo_user_id(
        &self,
        wahoo_user_id: i64,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let items = self.items.clone();
        Box::pin(async move {
            Ok(items
                .lock()
                .unwrap()
                .values()
                .find(|settings| settings.wahoo.user_id == Some(wahoo_user_id))
                .cloned())
        })
    }

    fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> crate::domain::settings::BoxFuture<Result<Vec<WahooUserIdBackfillCandidate>, SettingsError>>
    {
        let items = self.items.clone();
        Box::pin(async move {
            Ok(items
                .lock()
                .unwrap()
                .values()
                .filter(|settings| {
                    settings.wahoo.connected
                        && settings.wahoo.user_id.is_none()
                        && settings
                            .wahoo
                            .refresh_token
                            .as_deref()
                            .is_some_and(|value| !value.trim().is_empty())
                })
                .cloned()
                .map(|settings| WahooUserIdBackfillCandidate {
                    user_id: settings.user_id,
                    wahoo: settings.wahoo,
                })
                .collect())
        })
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

    fn get_authenticated_user(
        &self,
        _access_token: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooUser, WahooError>> {
        Box::pin(async { Ok(WahooUser { id: 60_462 }) })
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

mod import_mapping_tests {
    use super::*;
    use crate::domain::wahoo::{map_workout_to_import_command, WahooFileReference};

    fn sample_workout() -> WahooWorkout {
        WahooWorkout {
            id: 56_519,
            starts: "2023-11-14T08:00:00.000Z".to_string(),
            minutes: Some(60),
            name: Some("Wahoo Ride".to_string()),
            plan_id: None,
            plan_ids: Vec::new(),
            route_id: None,
            workout_token: Some("token-1".to_string()),
            workout_type_id: Some(0),
            workout_summary: Some(WahooWorkoutSummary {
                id: 8_297,
                name: Some("Wahoo Ride".to_string()),
                ascent_meters: Some(450.0),
                cadence_avg_rpm: Some(50.0),
                calories: Some(1500.0),
                distance_meters: Some(24_909.71),
                duration_active_seconds: Some(179.0),
                duration_paused_seconds: Some(95.0),
                duration_total_seconds: Some(275.0),
                heart_rate_avg_bpm: Some(100.0),
                normalized_power_watts: Some(150.0),
                training_stress_score: Some(304.9),
                average_power_watts: Some(94.59),
                speed_avg_mps: Some(10.75),
                total_work_joules: Some(1_041_480.0),
                time_zone: Some("America/Denver".to_string()),
                manual: false,
                edited: false,
                fitness_app_id: Some(1002),
                file: Some(WahooFileReference {
                    url: "https://example.test/file.fit".to_string(),
                }),
                created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
                updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
            }),
            created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
            updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
        }
    }

    #[test]
    fn map_workout_to_import_command_uses_wahoo_canonical_identity() {
        let mut workout = sample_workout();
        workout.plan_id = Some(7001);
        let command = map_workout_to_import_command("user-1", &workout)
            .expect("workout with summary should map");

        let crate::domain::external_sync::ExternalImportCommand::UpsertCompletedWorkout(import) =
            command
        else {
            panic!("expected completed workout import");
        };

        assert_eq!(import.workout.completed_workout_id, "wahoo-workout:56519");
        assert_eq!(import.workout.source_activity_id.as_deref(), Some("56519"));
        assert_eq!(import.workout.external_id.as_deref(), Some("56519"));
        assert_eq!(import.wahoo_workout_token.as_deref(), Some("token-1"));
        assert_eq!(import.wahoo_plan_id, Some(7001));
        assert_eq!(
            import.workout.details_unavailable_reason.as_deref(),
            Some("Detailed Wahoo workout data is still being processed. Please check back soon.")
        );
    }

    #[test]
    fn map_workout_to_import_command_uses_workout_id_not_summary_id_for_external_identity() {
        let mut workout = sample_workout();
        workout.id = 451_769_692;
        workout.workout_token = Some("icu_107574759".to_string());
        workout.workout_summary.as_mut().unwrap().id = 402_756_448;

        let command = map_workout_to_import_command("user-1", &workout)
            .expect("workout with summary should map");

        let crate::domain::external_sync::ExternalImportCommand::UpsertCompletedWorkout(import) =
            command
        else {
            panic!("expected completed workout import");
        };

        assert_eq!(import.external_id, "451769692");
        assert_eq!(
            import.workout.completed_workout_id,
            "wahoo-workout:451769692"
        );
        assert_eq!(
            import.workout.source_activity_id.as_deref(),
            Some("451769692")
        );
        assert_eq!(import.workout.external_id.as_deref(), Some("451769692"));
        assert_eq!(import.wahoo_workout_token.as_deref(), Some("icu_107574759"));
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

#[tokio::test]
async fn finish_connect_persists_wahoo_user_id() {
    let settings = InMemorySettingsRepository::default();
    let connect_states = InMemoryConnectStates::default();
    let oauth = TestOAuth::default();
    let service = WahooService::new(
        settings.clone(),
        connect_states.clone(),
        oauth,
        TestClock,
        TestIds,
    );

    connect_states
        .create(WahooConnectState::new(
            "state-1".to_string(),
            "user-1".to_string(),
            Some("/settings".to_string()),
            200,
            100,
        ))
        .await
        .unwrap();

    service
        .finish_connect("user-1", "state-1", "oauth-code")
        .await
        .unwrap();

    let stored = settings
        .find_by_user_id("user-1")
        .await
        .unwrap()
        .expect("settings should be stored");

    assert_eq!(stored.wahoo.user_id, Some(60_462));
}
