use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
        CompletedWorkoutRepository, CompletedWorkoutSeries, CompletedWorkoutStream,
        CompletedWorkoutZoneTime,
    },
    identity::Clock,
    intervals::{
        DateRange, PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
        PlannedWorkoutTarget, PlannedWorkoutText,
    },
    planned_workouts::{
        PlannedWorkout as CanonicalPlannedWorkout,
        PlannedWorkoutContent as CanonicalPlannedWorkoutContent,
        PlannedWorkoutLine as CanonicalPlannedWorkoutLine, PlannedWorkoutRepository,
        PlannedWorkoutStep as CanonicalPlannedWorkoutStep,
        PlannedWorkoutStepKind as CanonicalPlannedWorkoutStepKind,
        PlannedWorkoutTarget as CanonicalPlannedWorkoutTarget,
        PlannedWorkoutText as CanonicalPlannedWorkoutText,
    },
    races::{
        BoxFuture as RaceBoxFuture, Race, RaceDiscipline, RaceError, RacePriority, RaceRepository,
    },
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, Weekday,
    },
    special_days::{SpecialDay, SpecialDayKind, SpecialDayRepository},
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
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults("user-1".to_string(), 1);
            settings.cycling = CyclingSettings {
                full_name: Some("Alex".to_string()),
                ftp_watts: Some(300),
                athlete_prompt: Some("prefers concise coaching".to_string()),
                ..CyclingSettings::default()
            };
            settings.availability = test_availability();
            Ok(Some(settings))
        })
    }

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
            settings.availability = test_availability();
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

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
}

fn test_availability() -> AvailabilitySettings {
    AvailabilitySettings {
        configured: true,
        days: vec![
            AvailabilityDay {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Tue,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Wed,
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDay {
                weekday: Weekday::Thu,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Fri,
                available: true,
                max_duration_minutes: Some(120),
            },
            AvailabilityDay {
                weekday: Weekday::Sat,
                available: true,
                max_duration_minutes: Some(180),
            },
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
        ],
    }
}

#[derive(Clone)]
pub(super) struct TestWorkoutSummaryRepository;

#[derive(Clone)]
pub(super) struct TestCompletedWorkoutRepository {
    workouts: Vec<CompletedWorkout>,
}

impl Default for TestCompletedWorkoutRepository {
    fn default() -> Self {
        Self {
            workouts: vec![sample_completed_workout_on_date_with_ftp(
                "ride-1",
                "2026-04-03T08:00:00",
                Some(300),
                Some("intervals-event:101".to_string()),
            )],
        }
    }
}

impl TestCompletedWorkoutRepository {
    pub(super) fn with_workouts(workouts: Vec<CompletedWorkout>) -> Self {
        Self { workouts }
    }
}

impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .collect())
        })
    }

    fn upsert(
        &self,
        _workout: CompletedWorkout,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct TestPlannedWorkoutRepository {
    workouts: Vec<CanonicalPlannedWorkout>,
}

impl Default for TestPlannedWorkoutRepository {
    fn default() -> Self {
        Self {
            workouts: vec![
                sample_planned_workout(101, "2026-04-03"),
                sample_planned_workout(303, "2026-04-25"),
            ],
        }
    }
}

impl PlannedWorkoutRepository for TestPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<Vec<CanonicalPlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<Vec<CanonicalPlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .collect())
        })
    }

    fn upsert(
        &self,
        _workout: CanonicalPlannedWorkout,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<CanonicalPlannedWorkout, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct TestSpecialDayRepository {
    days: Vec<SpecialDay>,
}

impl Default for TestSpecialDayRepository {
    fn default() -> Self {
        Self {
            days: vec![SpecialDay::new(
                "intervals-special-day:202".to_string(),
                "user-1".to_string(),
                "2026-04-02".to_string(),
                SpecialDayKind::Note,
                Some("Sick day".to_string()),
                Some("Felt unwell with sore throat".to_string()),
            )
            .unwrap()],
        }
    }
}

impl SpecialDayRepository for TestSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::special_days::BoxFuture<
        Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>,
    > {
        let days = self.days.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::special_days::BoxFuture<
        Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>,
    > {
        let days = self.days.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id)
                .filter(|day| day.date >= oldest && day.date <= newest)
                .collect())
        })
    }

    fn upsert(
        &self,
        _special_day: SpecialDay,
    ) -> crate::domain::special_days::BoxFuture<
        Result<SpecialDay, crate::domain::special_days::SpecialDayError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(super) struct TestRaceRepository;

impl RaceRepository for TestRaceRepository {
    fn list_by_user_id(&self, user_id: &str) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(vec![Race {
                race_id: "race-1".to_string(),
                user_id,
                date: "2026-05-10".to_string(),
                name: "Spring Classic".to_string(),
                distance_meters: 123_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
                result: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }])
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(vec![Race {
                race_id: "race-1".to_string(),
                user_id,
                date: "2026-05-10".to_string(),
                name: "Spring Classic".to_string(),
                distance_meters: 123_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
                result: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }]
            .into_iter()
            .filter(|race| race.date >= oldest && race.date <= newest)
            .collect())
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        _user_id: &str,
        _race_id: &str,
    ) -> RaceBoxFuture<Result<Option<Race>, RaceError>> {
        unreachable!()
    }

    fn upsert(&self, _race: Race) -> RaceBoxFuture<Result<Race, RaceError>> {
        unreachable!()
    }

    fn delete(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        unreachable!()
    }
}

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

    fn find_active_by_user_id_and_operation_key(
        &self,
        _user_id: &str,
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

pub(super) fn sample_completed_workout_on_date_with_ftp(
    id: &str,
    start_date_local: &str,
    ftp_watts: Option<i32>,
    planned_workout_id: Option<String>,
) -> CompletedWorkout {
    CompletedWorkout {
        completed_workout_id: format!("intervals-activity:{id}"),
        user_id: "user-1".to_string(),
        start_date_local: start_date_local.to_string(),
        planned_workout_id,
        name: Some("Sweet Spot".to_string()),
        description: None,
        activity_type: Some("Ride".to_string()),
        duration_seconds: Some(3600),
        distance_meters: None,
        metrics: CompletedWorkoutMetrics {
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
        details: CompletedWorkoutDetails {
            intervals: vec![
                crate::domain::completed_workouts::CompletedWorkoutInterval {
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
                crate::domain::completed_workouts::CompletedWorkoutInterval {
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
                CompletedWorkoutStream {
                    stream_type: "watts".to_string(),
                    name: None,
                    primary_series: Some(CompletedWorkoutSeries::Integers(vec![
                        200, 220, 240, 260, 280,
                    ])),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                CompletedWorkoutStream {
                    stream_type: "cadence".to_string(),
                    name: None,
                    primary_series: Some(CompletedWorkoutSeries::Integers(vec![
                        80, 82, 84, 86, 88,
                    ])),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
            ],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z4".to_string(),
                seconds: 1200,
            }],
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    }
}

pub(super) fn sample_planned_workout(event_id: i64, date: &str) -> CanonicalPlannedWorkout {
    let (name, description, workout_doc) = if event_id == 303 {
        (
            "Long Tempo",
            Some("Endurance with tempo finish".to_string()),
            "- 90m 75%",
        )
    } else {
        ("Sweet Spot", None, "- 2x10min 90-95%")
    };

    CanonicalPlannedWorkout::new(
        format!("intervals-event:{event_id}"),
        "user-1".to_string(),
        date.to_string(),
        CanonicalPlannedWorkoutContent {
            lines: vec![
                CanonicalPlannedWorkoutLine::Text(CanonicalPlannedWorkoutText {
                    text: name.to_string(),
                }),
                CanonicalPlannedWorkoutLine::Step(CanonicalPlannedWorkoutStep {
                    duration_seconds: if event_id == 303 { 5400 } else { 1200 },
                    kind: CanonicalPlannedWorkoutStepKind::Steady,
                    target: CanonicalPlannedWorkoutTarget::PercentFtp {
                        min: if event_id == 303 { 75.0 } else { 90.0 },
                        max: if event_id == 303 { 75.0 } else { 95.0 },
                    },
                }),
            ],
        },
    )
    .with_event_metadata(
        Some(name.to_string()),
        description.or_else(|| Some(workout_doc.to_string()).filter(|_| event_id != 101)),
        Some("Ride".to_string()),
    )
}
