use crate::domain::{
    identity::Clock,
    intervals::{
        Activity, ActivityDetails, ActivityMetrics, ActivityStream, DateRange, Event,
        EventCategory, IntervalsError, IntervalsUseCases, PlannedWorkout, PlannedWorkoutLine,
        PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    },
    settings::{
        AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError,
        UserSettings, UserSettingsUseCases,
    },
    training_plan::{
        TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
        TrainingPlanSnapshot,
    },
    workout_summary::{
        ConversationMessage, MessageRole, WorkoutRecap, WorkoutSummary, WorkoutSummaryError,
        WorkoutSummaryRepository,
    },
};

#[derive(Clone)]
pub(super) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_775_174_400
    }
}

#[derive(Clone)]
pub(super) struct TestSettingsService;

impl UserSettingsUseCases for TestSettingsService {
    fn get_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults("user-1".to_string(), 1);
            settings.cycling = CyclingSettings {
                full_name: Some("Alex".to_string()),
                ftp_watts: Some(300),
                athlete_prompt: Some("prefers concise coaching".to_string()),
                ..CyclingSettings::default()
            };
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct TestIntervalsService;

impl IntervalsUseCases for TestIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Event>, IntervalsError>> {
        Box::pin(async move {
            Ok(vec![
                Event {
                    id: 101,
                    start_date_local: "2026-04-03T07:00:00".to_string(),
                    name: Some("Sweet Spot".to_string()),
                    category: EventCategory::Workout,
                    description: None,
                    indoor: false,
                    color: None,
                    workout_doc: Some("- 2x10min 90-95%".to_string()),
                },
                Event {
                    id: 202,
                    start_date_local: "2026-04-02T09:00:00".to_string(),
                    name: Some("Sick day".to_string()),
                    category: EventCategory::Note,
                    description: Some("Felt unwell with sore throat".to_string()),
                    indoor: false,
                    color: None,
                    workout_doc: None,
                },
            ])
        })
    }

    fn get_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn create_event(
        &self,
        _user_id: &str,
        _event: crate::domain::intervals::CreateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn update_event(
        &self,
        _user_id: &str,
        _event_id: i64,
        _event: crate::domain::intervals::UpdateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn delete_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        unreachable!()
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<u8>, IntervalsError>> {
        unreachable!()
    }

    fn list_activities(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        Box::pin(async move { Ok(vec![sample_activity_with_ftp(Some(300))]) })
    }

    fn get_activity(
        &self,
        _user_id: &str,
        _activity_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
        Box::pin(async move { Ok(sample_activity_with_ftp(Some(300))) })
    }

    fn upload_activity(
        &self,
        _user_id: &str,
        _upload: crate::domain::intervals::UploadActivity,
    ) -> crate::domain::intervals::BoxFuture<
        Result<crate::domain::intervals::UploadedActivities, IntervalsError>,
    > {
        unreachable!()
    }

    fn update_activity(
        &self,
        _user_id: &str,
        _activity_id: &str,
        _activity: crate::domain::intervals::UpdateActivity,
    ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
        unreachable!()
    }

    fn delete_activity(
        &self,
        _user_id: &str,
        _activity_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct TestWorkoutSummaryRepository;

fn summary_for_workout_id(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(7),
        messages: vec![ConversationMessage {
            id: "message-1".to_string(),
            role: MessageRole::User,
            content: "felt controlled".to_string(),
            created_at_epoch_seconds: 1,
        }],
        saved_at_epoch_seconds: None,
        workout_recap_text: Some("Strong sweet spot execution with steady control".to_string()),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some("test-model".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

impl WorkoutSummaryRepository for TestWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Option<WorkoutSummary>, WorkoutSummaryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        _user_id: &str,
        workout_ids: Vec<String>,
    ) -> crate::domain::workout_summary::BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>
    {
        Box::pin(async move {
            Ok(workout_ids
                .into_iter()
                .filter(|id| id == "ride-1" || id == "101")
                .map(|id| summary_for_workout_id(&id))
                .collect())
        })
    }

    fn create(
        &self,
        _summary: WorkoutSummary,
    ) -> crate::domain::workout_summary::BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>
    {
        unreachable!()
    }

    fn update_rpe(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _rpe: u8,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn append_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message: ConversationMessage,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn set_saved_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: Option<i64>,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap: WorkoutRecap,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn find_message_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Option<ConversationMessage>, WorkoutSummaryError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct TestTrainingPlanProjectionRepository;

impl TrainingPlanProjectionRepository for TestTrainingPlanProjectionRepository {
    fn list_active_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        Box::pin(async move {
            Ok(vec![
                TrainingPlanProjectedDay {
                    user_id: "user-1".to_string(),
                    workout_id: "ride-1".to_string(),
                    operation_key: "training-plan:user-1:ride-1:1775174400".to_string(),
                    date: "2026-04-04".to_string(),
                    rest_day: false,
                    workout: Some(PlannedWorkout {
                        lines: vec![
                            PlannedWorkoutLine::Text(PlannedWorkoutText {
                                text: "Past AI Threshold".to_string(),
                            }),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 600,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 92.0,
                                    max: 97.0,
                                },
                            }),
                        ],
                    }),
                    superseded_at_epoch_seconds: None,
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 1,
                },
                TrainingPlanProjectedDay {
                    user_id: "user-1".to_string(),
                    workout_id: "ride-1".to_string(),
                    operation_key: "training-plan:user-1:ride-1:1775174400".to_string(),
                    date: "2026-04-07".to_string(),
                    rest_day: false,
                    workout: Some(PlannedWorkout {
                        lines: vec![
                            PlannedWorkoutLine::Text(PlannedWorkoutText {
                                text: "AI Threshold".to_string(),
                            }),
                            PlannedWorkoutLine::Repeat(
                                crate::domain::intervals::PlannedWorkoutRepeat {
                                    title: Some("Main Set".to_string()),
                                    count: 2,
                                },
                            ),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 600,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 92.0,
                                    max: 97.0,
                                },
                            }),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 180,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 55.0,
                                    max: 55.0,
                                },
                            }),
                            PlannedWorkoutLine::Text(PlannedWorkoutText {
                                text: "Cooldown".to_string(),
                            }),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 300,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 50.0,
                                    max: 50.0,
                                },
                            }),
                        ],
                    }),
                    superseded_at_epoch_seconds: None,
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 1,
                },
            ])
        })
    }

    fn find_active_by_operation_key(
        &self,
        _operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        unreachable!()
    }

    fn replace_window(
        &self,
        _snapshot: TrainingPlanSnapshot,
        _projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct FtpOrderingIntervalsService;

impl IntervalsUseCases for FtpOrderingIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Event>, IntervalsError>> {
        Box::pin(async move {
            Ok(vec![Event {
                id: 101,
                start_date_local: "2026-04-03T07:00:00".to_string(),
                name: Some("Workout match".to_string()),
                category: EventCategory::Workout,
                description: None,
                indoor: false,
                color: None,
                workout_doc: Some("- 2x10min 90-95%".to_string()),
            }])
        })
    }

    fn get_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn create_event(
        &self,
        _user_id: &str,
        _event: crate::domain::intervals::CreateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn update_event(
        &self,
        _user_id: &str,
        _event_id: i64,
        _event: crate::domain::intervals::UpdateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn delete_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        unreachable!()
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<u8>, IntervalsError>> {
        unreachable!()
    }

    fn list_activities(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        Box::pin(async move {
            Ok(vec![
                sample_activity_on_date_with_ftp("ride-late", "2026-04-03T08:00:00", Some(320)),
                sample_activity_on_date_with_ftp("ride-early", "2026-03-15T08:00:00", Some(280)),
            ])
        })
    }

    fn get_activity(
        &self,
        _user_id: &str,
        _activity_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
        Box::pin(async move {
            Ok(sample_activity_on_date_with_ftp(
                "ride-late",
                "2026-04-03T08:00:00",
                Some(320),
            ))
        })
    }

    fn upload_activity(
        &self,
        _user_id: &str,
        _upload: crate::domain::intervals::UploadActivity,
    ) -> crate::domain::intervals::BoxFuture<
        Result<crate::domain::intervals::UploadedActivities, IntervalsError>,
    > {
        unreachable!()
    }

    fn update_activity(
        &self,
        _user_id: &str,
        _activity_id: &str,
        _activity: crate::domain::intervals::UpdateActivity,
    ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
        unreachable!()
    }

    fn delete_activity(
        &self,
        _user_id: &str,
        _activity_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        unreachable!()
    }
}

pub(super) fn sample_activity_on_date_with_ftp(
    id: &str,
    start_date_local: &str,
    ftp_watts: Option<i32>,
) -> Activity {
    let mut activity = sample_activity_with_ftp(ftp_watts);
    activity.id = id.to_string();
    activity.start_date_local = start_date_local.to_string();
    activity
}

pub(super) fn sample_activity_with_ftp(ftp_watts: Option<i32>) -> Activity {
    Activity {
        id: "ride-1".to_string(),
        athlete_id: None,
        start_date_local: "2026-04-03T08:00:00".to_string(),
        start_date: None,
        name: Some("Sweet Spot".to_string()),
        description: None,
        activity_type: Some("Ride".to_string()),
        source: None,
        external_id: None,
        device_name: None,
        distance_meters: None,
        moving_time_seconds: Some(3600),
        elapsed_time_seconds: Some(3600),
        total_elevation_gain_meters: None,
        total_elevation_loss_meters: None,
        average_speed_mps: None,
        max_speed_mps: None,
        average_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        average_cadence_rpm: None,
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: false,
        stream_types: vec!["watts".to_string(), "cadence".to_string()],
        tags: Vec::new(),
        metrics: ActivityMetrics {
            training_stress_score: Some(80),
            normalized_power_watts: Some(250),
            intensity_factor: Some(0.83),
            efficiency_factor: Some(1.2),
            variability_index: Some(1.05),
            average_power_watts: Some(238),
            ftp_watts,
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: ActivityDetails {
            intervals: vec![
                crate::domain::intervals::ActivityInterval {
                    id: Some(1),
                    label: Some("Work 1".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("g1".to_string()),
                    start_index: Some(600),
                    end_index: Some(1200),
                    start_time_seconds: Some(600),
                    end_time_seconds: Some(1200),
                    moving_time_seconds: Some(600),
                    elapsed_time_seconds: Some(600),
                    distance_meters: None,
                    average_power_watts: Some(278),
                    normalized_power_watts: Some(280),
                    training_stress_score: Some(20.0),
                    average_heart_rate_bpm: None,
                    average_cadence_rpm: None,
                    average_speed_mps: None,
                    average_stride_meters: None,
                    zone: Some(4),
                },
                crate::domain::intervals::ActivityInterval {
                    id: Some(2),
                    label: Some("Work 2".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("g1".to_string()),
                    start_index: Some(1500),
                    end_index: Some(2100),
                    start_time_seconds: Some(1500),
                    end_time_seconds: Some(2100),
                    moving_time_seconds: Some(600),
                    elapsed_time_seconds: Some(600),
                    distance_meters: None,
                    average_power_watts: Some(279),
                    normalized_power_watts: Some(281),
                    training_stress_score: Some(20.0),
                    average_heart_rate_bpm: None,
                    average_cadence_rpm: None,
                    average_speed_mps: None,
                    average_stride_meters: None,
                    zone: Some(4),
                },
            ],
            interval_groups: Vec::new(),
            streams: vec![
                ActivityStream {
                    stream_type: "watts".to_string(),
                    name: None,
                    data: Some(serde_json::json!([200, 220, 240, 260, 280])),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                ActivityStream {
                    stream_type: "cadence".to_string(),
                    name: None,
                    data: Some(serde_json::json!([80, 82, 84, 86, 88])),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
            ],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}
