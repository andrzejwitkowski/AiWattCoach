use std::sync::{Arc, Mutex};

use crate::domain::{
    external_sync::{
        BoxFuture as ExternalSyncBoxFuture, ExternalImportCommand, ExternalImportError,
        ExternalImportOutcome, ExternalImportUseCases,
    },
    identity::Clock,
    settings::{
        SettingsError, UserSettings, UserSettingsRepository, WahooConfig,
        WahooUserIdBackfillCandidate,
    },
    training_load::{
        BoxFuture as TrainingLoadBoxFuture, TrainingLoadError, TrainingLoadRecomputeUseCases,
    },
    wahoo_fit_enrichment::{
        BoxFuture as WahooFitEnrichmentBoxFuture, WahooFitEnrichmentError,
        WahooFitEnrichmentQueueUseCases,
    },
};

use super::*;
use crate::domain::wahoo::{
    WahooAuthExchange, WahooAuthStart, WahooCreatePlan, WahooCreateWorkout, WahooPlan, WahooToken,
    WahooUpdatePlan, WahooUpdateWorkout, WahooUser, WahooWorkoutList, WahooWorkoutSummary,
};

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
struct InMemorySettingsRepository {
    items: Arc<Mutex<Vec<UserSettings>>>,
}

impl UserSettingsRepository for InMemorySettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let items = self.items.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(items
                .lock()
                .unwrap()
                .iter()
                .find(|settings| settings.user_id == user_id)
                .cloned())
        })
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
                .iter()
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
                .iter()
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
            let mut items = items.lock().unwrap();
            if let Some(existing) = items
                .iter_mut()
                .find(|item| item.user_id == settings.user_id)
            {
                *existing = settings.clone();
            } else {
                items.push(settings.clone());
            }
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: crate::domain::settings::AiAgentsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: crate::domain::settings::IntervalsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: crate::domain::settings::AnalysisOptions,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: crate::domain::settings::CyclingSettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: crate::domain::settings::AvailabilitySettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }
}

#[derive(Clone, Default)]
struct RecordingImportService {
    commands: Arc<Mutex<Vec<ExternalImportCommand>>>,
}

impl RecordingImportService {
    fn commands(&self) -> Vec<ExternalImportCommand> {
        self.commands.lock().unwrap().clone()
    }
}

impl ExternalImportUseCases for RecordingImportService {
    fn import(
        &self,
        command: ExternalImportCommand,
    ) -> ExternalSyncBoxFuture<Result<ExternalImportOutcome, ExternalImportError>> {
        let commands = self.commands.clone();
        Box::pin(async move {
            commands.lock().unwrap().push(command.clone());
            let ExternalImportCommand::UpsertCompletedWorkout(import) = command else {
                panic!("expected completed workout import");
            };
            Ok(ExternalImportOutcome {
                canonical_entity: crate::domain::external_sync::CanonicalEntityRef::new(
                    crate::domain::external_sync::CanonicalEntityKind::CompletedWorkout,
                    import.workout.completed_workout_id,
                ),
                provider: import.provider,
                external_id: import.external_id,
            })
        })
    }
}

#[derive(Clone, Default)]
struct RecordingTrainingLoadService {
    calls: Arc<Mutex<Vec<(String, String, i64)>>>,
}

impl RecordingTrainingLoadService {
    fn calls(&self) -> Vec<(String, String, i64)> {
        self.calls.lock().unwrap().clone()
    }
}

impl TrainingLoadRecomputeUseCases for RecordingTrainingLoadService {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> TrainingLoadBoxFuture<Result<(), TrainingLoadError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest_date = oldest_date.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest_date, now_epoch_seconds));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct RecordingQueue {
    calls: Arc<Mutex<Vec<(String, String, i64)>>>,
}

impl RecordingQueue {
    fn calls(&self) -> Vec<(String, String, i64)> {
        self.calls.lock().unwrap().clone()
    }
}

impl WahooFitEnrichmentQueueUseCases for RecordingQueue {
    fn enqueue_enrichment(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> WahooFitEnrichmentBoxFuture<Result<(), WahooFitEnrichmentError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, completed_workout_id, wahoo_workout_id));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct RecordingWahooService {
    workouts: Arc<Mutex<Vec<WahooWorkout>>>,
}

impl WahooUseCases for RecordingWahooService {
    fn begin_connect(
        &self,
        _user_id: &str,
        _return_to: Option<String>,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooAuthStart, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn finish_connect(
        &self,
        _user_id: &str,
        _state: &str,
        _code: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooAuthExchange, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn ensure_token(
        &self,
        _user_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooToken, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_authenticated_user(
        &self,
        _user_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooUser, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        page: usize,
        per_page: usize,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkoutList, WahooError>> {
        let workouts = self.workouts.clone();
        Box::pin(async move {
            let workouts = workouts.lock().unwrap().clone();
            let start = (page.saturating_sub(1)) * per_page;
            Ok(WahooWorkoutList {
                total: workouts.len(),
                workouts: workouts.into_iter().skip(start).take(per_page).collect(),
                page,
                per_page,
                order: None,
                sort: None,
            })
        })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotFound) })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<Option<WahooPlan>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        _request: WahooCreatePlan,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        _plan_id: i64,
        _request: WahooUpdatePlan,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        _request: WahooCreateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_workout(
        &self,
        _user_id: &str,
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

fn sample_settings(user_id: &str, wahoo_user_id: i64) -> UserSettings {
    let mut settings = UserSettings::new_defaults(user_id.to_string(), 1);
    settings.wahoo = WahooConfig {
        user_id: Some(wahoo_user_id),
        connected: true,
        ..WahooConfig::default()
    };
    settings
}

fn sample_workout(id: i64, starts: &str, with_fit_file: bool) -> WahooWorkout {
    WahooWorkout {
        id,
        starts: starts.to_string(),
        minutes: Some(60),
        name: Some(format!("Workout {id}")),
        plan_id: None,
        plan_ids: Vec::new(),
        route_id: None,
        workout_token: Some(format!("token-{id}")),
        workout_type_id: Some(12),
        workout_summary: Some(WahooWorkoutSummary {
            id,
            name: Some(format!("Workout {id}")),
            ascent_meters: None,
            cadence_avg_rpm: None,
            calories: Some(500.0),
            distance_meters: Some(20_000.0),
            duration_active_seconds: Some(3600.0),
            duration_paused_seconds: Some(0.0),
            duration_total_seconds: Some(3600.0),
            heart_rate_avg_bpm: None,
            normalized_power_watts: Some(220.0),
            training_stress_score: Some(80.0),
            average_power_watts: Some(200.0),
            speed_avg_mps: None,
            total_work_joules: None,
            time_zone: None,
            manual: false,
            edited: false,
            fitness_app_id: None,
            file: with_fit_file.then(|| crate::domain::wahoo::WahooFileReference {
                url: format!("https://example.test/{id}.fit"),
            }),
            created_at: Some(starts.to_string()),
            updated_at: Some(starts.to_string()),
        }),
        created_at: Some(starts.to_string()),
        updated_at: Some(starts.to_string()),
    }
}

#[tokio::test]
async fn import_webhook_workout_ignores_unknown_wahoo_user() {
    let service = WahooWebhookService::new(
        InMemorySettingsRepository::default(),
        RecordingImportService::default(),
        RecordingWahooService::default(),
        RecordingTrainingLoadService::default(),
        RecordingQueue::default(),
        FixedClock,
        Some("secret".to_string()),
    );

    let outcome = service
        .import_webhook_workout(
            "secret",
            1234,
            sample_workout(42, "2023-11-14T08:00:00Z", true),
        )
        .await
        .unwrap();

    assert_eq!(outcome, WahooWebhookOutcome::Ignored);
}

#[tokio::test]
async fn import_webhook_workout_imports_and_enqueues_fit_details() {
    let settings = InMemorySettingsRepository::default();
    settings
        .upsert(sample_settings("user-1", 60_462))
        .await
        .unwrap();
    let imports = RecordingImportService::default();
    let queue = RecordingQueue::default();
    let training_load = RecordingTrainingLoadService::default();
    let service = WahooWebhookService::new(
        settings,
        imports.clone(),
        RecordingWahooService::default(),
        training_load.clone(),
        queue.clone(),
        FixedClock,
        Some("secret".to_string()),
    );

    let outcome = service
        .import_webhook_workout(
            "secret",
            60_462,
            sample_workout(42, "2023-11-14T08:00:00Z", true),
        )
        .await
        .unwrap();

    assert_eq!(
        outcome,
        WahooWebhookOutcome::Accepted(WahooWebhookAccepted {
            user_id: "user-1".to_string(),
            completed_workout_id: "wahoo-workout:42".to_string(),
        })
    );
    assert_eq!(imports.commands().len(), 1);
    assert_eq!(
        queue.calls(),
        vec![("user-1".to_string(), "wahoo-workout:42".to_string(), 42,)]
    );
    assert_eq!(
        training_load.calls(),
        vec![(
            "user-1".to_string(),
            "2023-11-14".to_string(),
            1_700_000_000,
        )]
    );
}

#[tokio::test]
async fn sync_completed_workouts_for_user_imports_all_pages_and_recomputes_earliest_date() {
    let wahoo = RecordingWahooService {
        workouts: Arc::new(Mutex::new(vec![
            sample_workout(43, "2023-11-15T08:00:00Z", false),
            sample_workout(42, "2023-11-14T08:00:00Z", true),
        ])),
    };
    let imports = RecordingImportService::default();
    let queue = RecordingQueue::default();
    let training_load = RecordingTrainingLoadService::default();
    let service = WahooWebhookService::new(
        InMemorySettingsRepository::default(),
        imports.clone(),
        wahoo,
        training_load.clone(),
        queue.clone(),
        FixedClock,
        None,
    );

    let result = service
        .sync_completed_workouts_for_user("user-1")
        .await
        .unwrap();

    assert_eq!(
        result,
        ManualWahooSyncResult {
            scanned: 2,
            imported: 2,
            skipped: 0,
        }
    );
    assert_eq!(imports.commands().len(), 2);
    assert_eq!(queue.calls().len(), 1);
    assert_eq!(
        training_load.calls(),
        vec![(
            "user-1".to_string(),
            "2023-11-14".to_string(),
            1_700_000_000,
        )]
    );
}
