//! Seeded preview path: explicit planned blocks + Wahoo FIT raw streams → preview must
//! compute the same adjusted intervals as `align_workout_from_blocks` (not merely emit `sa`).

use std::sync::Arc;

use crate::domain::{
    admin_prompt_preview::{AdminPromptPreviewService, AdminPromptPreviewUseCases},
    calendar_view::NoopPlannedCompletedWorkoutLinkRepository,
    completed_workouts::{
        AuthoritativeCompletedWorkoutRepository, CompletedWorkoutReadService,
        CompletedWorkoutRepository, CompletedWorkoutSeries, CompletedWorkoutStream,
    },
    llm::{LlmError, LlmProvider, LlmProviderConfig, UserLlmConfigProvider},
    planned_workouts::{
        AuthoritativePlannedWorkoutRepository, PlannedWorkout as CanonicalPlannedWorkout,
        PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
        PlannedWorkoutTarget,
    },
    training_context::{DefaultTrainingContextBuilder, TrainingContextBuilder},
    training_plan::{TrainingPlanError, WorkoutPlanningLlmConfigPort},
    workout_alignment::{align_workout_from_blocks, AlignedInterval, PlannedBlockInput},
    workout_summary::{
        CompletedWorkoutAliasScope, CompletedWorkoutTargetUseCases, ResolvedCompletedWorkoutTarget,
        WorkoutChatLlmConfigPort, WorkoutSummaryError,
    },
};

use super::super::support::{
    sample_completed_workout_on_date_with_ftp, FixedClock, TestCompletedWorkoutRepository,
    TestPlannedWorkoutRepository, TestSettingsService, TestSpecialDayRepository,
    TestWorkoutSummaryRepository,
};
use super::wahoo_sync_state;

const FTP_WATTS: i32 = 300;
const SOURCE_ACTIVITY_ID: &str = "i166368784";
const PLANNED_ID: &str = "intervals-event:101";
const FOCUS_DATE: &str = "2026-04-03";

/// Planned blocks the athlete was supposed to ride (synced to Intervals).
/// Names are omitted — production `compute_aligned_intervals` maps blocks with `name: None`
/// and the aligner labels them `step N`.
fn planned_blocks() -> Vec<PlannedBlockInput> {
    vec![
        PlannedBlockInput {
            name: None,
            duration_seconds: 5,
            min_percent_ftp: Some(90.0),
            max_percent_ftp: Some(95.0),
            min_target_watts: None,
            max_target_watts: None,
        },
        PlannedBlockInput {
            name: None,
            duration_seconds: 5,
            min_percent_ftp: Some(50.0),
            max_percent_ftp: Some(55.0),
            min_target_watts: None,
            max_target_watts: None,
        },
    ]
}

/// Second-by-second FIT streams from the Wahoo file (raw, not adjusted).
fn fit_raw_power() -> Vec<i32> {
    vec![280, 282, 278, 281, 279, 152, 155, 150, 154, 151]
}

fn fit_raw_cadence() -> Vec<i32> {
    vec![90, 91, 89, 90, 92, 78, 80, 79, 81, 77]
}

fn planned_workout_document() -> CanonicalPlannedWorkout {
    let blocks = planned_blocks();
    CanonicalPlannedWorkout::new(
        PLANNED_ID.to_string(),
        "user-1".to_string(),
        FOCUS_DATE.to_string(),
        PlannedWorkoutContent {
            lines: blocks
                .into_iter()
                .map(|block| {
                    PlannedWorkoutLine::Step(PlannedWorkoutStep {
                        duration_seconds: block.duration_seconds,
                        kind: PlannedWorkoutStepKind::Steady,
                        target: PlannedWorkoutTarget::PercentFtp {
                            min: block.min_percent_ftp.unwrap(),
                            max: block.max_percent_ftp.unwrap(),
                        },
                    })
                })
                .collect(),
        },
    )
    .with_event_metadata(
        Some("Sweet Spot + Recovery".to_string()),
        None,
        Some("Ride".to_string()),
    )
}

fn sample_llm_config() -> LlmProviderConfig {
    LlmProviderConfig {
        provider: LlmProvider::OpenAi,
        model: "gpt-4o-mini".to_string(),
        api_key: "test-key".to_string(),
    }
}

#[derive(Clone)]
struct StubLlmConfig;

impl UserLlmConfigProvider for StubLlmConfig {
    fn get_config(
        &self,
        _user_id: &str,
    ) -> crate::domain::llm::BoxFuture<Result<LlmProviderConfig, LlmError>> {
        Box::pin(async { Ok(sample_llm_config()) })
    }
}

impl WorkoutChatLlmConfigPort for StubLlmConfig {
    fn get_workout_chat_config(
        &self,
        _user_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<Result<LlmProviderConfig, WorkoutSummaryError>>
    {
        Box::pin(async { Ok(sample_llm_config()) })
    }
}

impl WorkoutPlanningLlmConfigPort for StubLlmConfig {
    fn get_workout_planning_config(
        &self,
        _user_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<Result<LlmProviderConfig, TrainingPlanError>> {
        Box::pin(async { Ok(sample_llm_config()) })
    }
}

/// Mirrors production CompletedWorkoutTargetAdapter: preferred_workout_id = source_activity_id.
#[derive(Clone)]
struct SourceActivityPreferredTarget {
    repository: TestCompletedWorkoutRepository,
}

impl CompletedWorkoutTargetUseCases for SourceActivityPreferredTarget {
    fn is_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<Result<bool, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let workouts = repository
                .list_by_user_id(&user_id)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            Ok(workouts.iter().any(|workout| {
                workout.completed_workout_id == workout_id
                    || workout.source_activity_id.as_deref() == Some(workout_id.as_str())
                    || workout.completed_workout_id.rsplit(':').next() == Some(workout_id.as_str())
            }))
        })
    }

    fn resolve_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Option<ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>,
    > {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let workouts = repository
                .list_by_user_id(&user_id)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            let Some(workout) = workouts.into_iter().find(|workout| {
                workout.completed_workout_id == workout_id
                    || workout.source_activity_id.as_deref() == Some(workout_id.as_str())
                    || workout.completed_workout_id.rsplit(':').next() == Some(workout_id.as_str())
            }) else {
                return Ok(None);
            };
            let preferred_workout_id = workout
                .source_activity_id
                .clone()
                .unwrap_or_else(|| workout.completed_workout_id.clone());
            Ok(Some(ResolvedCompletedWorkoutTarget {
                preferred_workout_id: preferred_workout_id.clone(),
                equivalent_workout_ids: vec![preferred_workout_id, workout.completed_workout_id],
            }))
        })
    }

    fn resolve_completed_workout_targets_in_scope(
        &self,
        user_id: &str,
        workout_ids: &[String],
        _alias_scope: &CompletedWorkoutAliasScope,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<
            std::collections::HashMap<String, ResolvedCompletedWorkoutTarget>,
            WorkoutSummaryError,
        >,
    > {
        let this = self.clone();
        let user_id = user_id.to_string();
        let workout_ids = workout_ids.to_vec();
        Box::pin(async move {
            let mut out = std::collections::HashMap::new();
            for workout_id in workout_ids {
                if let Some(resolved) = this
                    .resolve_completed_workout_target(&user_id, &workout_id)
                    .await?
                {
                    out.insert(workout_id, resolved);
                }
            }
            Ok(out)
        })
    }
}

fn set_fit_streams(
    workout: &mut crate::domain::completed_workouts::CompletedWorkout,
    power: &[i32],
    cadence: &[i32],
) {
    workout.details.streams = vec![
        CompletedWorkoutStream {
            stream_type: "watts".to_string(),
            name: None,
            primary_series: Some(CompletedWorkoutSeries::Integers(
                power.iter().map(|&v| i64::from(v)).collect(),
            )),
            secondary_series: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
        CompletedWorkoutStream {
            stream_type: "cadence".to_string(),
            name: None,
            primary_series: Some(CompletedWorkoutSeries::Integers(
                cadence.iter().map(|&v| i64::from(v)).collect(),
            )),
            secondary_series: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
    ];
}

#[tokio::test]
async fn preview_post_workout_calculates_adjusted_blocks_from_plan_and_wahoo_fit() {
    // --- 1) Planned blocks (what Intervals sync would store) ---
    let blocks = planned_blocks();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].duration_seconds, 5);
    assert_eq!(blocks[0].min_percent_ftp, Some(90.0));
    assert_eq!(blocks[1].duration_seconds, 5);
    assert_eq!(blocks[1].min_percent_ftp, Some(50.0));
    let planned = planned_workout_document();
    assert_eq!(planned.planned_workout_id, PLANNED_ID);
    assert_eq!(
        planned
            .workout
            .lines
            .iter()
            .filter(|line| matches!(line, PlannedWorkoutLine::Step(_)))
            .count(),
        2,
        "planned workout must expose two steps"
    );

    // --- 2) Wahoo FIT raw streams (not adjusted yet) ---
    let power = fit_raw_power();
    let cadence = fit_raw_cadence();
    assert_eq!(power.len(), 10, "FIT must provide second-by-second watts");
    assert_eq!(cadence.len(), 10);
    assert!(
        power.iter().any(|&w| w >= 270) && power.iter().any(|&w| w <= 160),
        "raw FIT must contain both work and recovery watts, not pre-adjusted blocks"
    );

    // Ground truth: same engine preview should invoke via training-context attach.
    let expected: Vec<AlignedInterval> =
        align_workout_from_blocks(&blocks, Some(FTP_WATTS), &power, &cadence)
            .expect("known plan+FIT inputs must produce adjusted intervals");
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[0].planned_step.target_power_min, 270); // 90% of 300
    assert_eq!(expected[0].planned_step.target_power_max, 285); // 95% of 300
    assert_eq!(expected[1].planned_step.target_power_min, 150); // 50% of 300
    assert_eq!(expected[1].planned_step.target_power_max, 165); // 55% of 300
    assert!(
        expected[0].avg_power > 200 && expected[1].avg_power < 200,
        "adjusted blocks must reflect work then recovery from raw FIT; got {:?}",
        expected.iter().map(|i| i.avg_power).collect::<Vec<_>>()
    );

    // --- 3) Seed end state: Intervals first (no FIT), Wahoo second (FIT + plan link) ---
    let mut intervals = sample_completed_workout_on_date_with_ftp(
        SOURCE_ACTIVITY_ID,
        "2026-04-03T08:00:00",
        None,
        None,
    );
    intervals.source_activity_id = Some(SOURCE_ACTIVITY_ID.to_string());
    intervals.details.streams.clear();

    let mut wahoo = sample_completed_workout_on_date_with_ftp(
        "476396735",
        "2026-04-03T08:00:00",
        None,
        Some(PLANNED_ID.to_string()),
    );
    wahoo.completed_workout_id = "wahoo-workout:476396735".to_string();
    wahoo.source_activity_id = Some(SOURCE_ACTIVITY_ID.to_string());
    set_fit_streams(&mut wahoo, &power, &cadence);
    assert!(
        wahoo
            .details
            .streams
            .iter()
            .any(|s| s.stream_type == "watts"),
        "Wahoo completed workout must carry FIT watts before preview"
    );

    let completed_repo = TestCompletedWorkoutRepository::with_workouts(vec![intervals, wahoo]);
    let planned_repo = TestPlannedWorkoutRepository::with_workouts(vec![planned]);
    let authoritative_completed = AuthoritativeCompletedWorkoutRepository::new(
        completed_repo.clone(),
        super::TestSyncStates {
            states: vec![wahoo_sync_state("wahoo-workout:476396735")],
        },
    );
    let authoritative_planned = AuthoritativePlannedWorkoutRepository::new(
        planned_repo.clone(),
        authoritative_completed.clone(),
        NoopPlannedCompletedWorkoutLinkRepository,
    );
    let target = Arc::new(SourceActivityPreferredTarget {
        repository: completed_repo.clone(),
    });

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        target.clone(),
        FixedClock,
    )
    .with_completed_workout_repository(authoritative_completed.clone())
    .with_planned_workout_repository(authoritative_planned)
    .with_unfiltered_planned_workout_repository(planned_repo.clone())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let llm = Arc::new(StubLlmConfig);
    let preview = AdminPromptPreviewService::new(
        Arc::new(builder) as Arc<dyn TrainingContextBuilder>,
        llm.clone(),
        llm.clone(),
        llm.clone(),
        Arc::new(CompletedWorkoutReadService::new(authoritative_completed)),
        Some(planned_repo),
        Some(TestSpecialDayRepository::default()),
        target,
        Arc::new(TestWorkoutSummaryRepository),
        None,
        Arc::new(TestSettingsService),
        None,
        None,
        None,
        None,
        None,
        FixedClock,
    );

    // --- 4) Simulate admin "Preview" click ---
    let response = preview
        .preview_post_workout("user-1", FOCUS_DATE)
        .await
        .expect("preview should succeed for seeded day");

    assert_eq!(
        response.meta.selected_workout_id.as_deref(),
        Some(SOURCE_ACTIVITY_ID)
    );

    let packed = response
        .request
        .volatile_context
        .split_once("training_context_volatile=")
        .map(|(_, json)| json)
        .expect("preview volatile_context must embed training_context_volatile JSON");
    let volatile: serde_json::Value =
        serde_json::from_str(packed).expect("training_context_volatile should be JSON");

    assert_eq!(volatile["fx"]["k"].as_str(), Some("activity"));

    // --- 5) Adjusted blocks from preview must equal the alignment of plan + FIT raw ---
    let expected_sa = serde_json::to_value(&expected).expect("expected intervals serialize");
    assert_eq!(
        volatile["sa"], expected_sa,
        "preview must calculate adjusted blocks from planned steps + Wahoo FIT raw streams"
    );

    // When adjusted blocks are attached, focus raw ps/cs must not be the evidence path.
    let packed_text = packed.to_string();
    assert!(
        !packed_text.contains("[[280,282,5]]") && !packed_text.contains("[[280,280,"),
        "raw FIT power segments must not remain as the packed evidence when sa is present; packed={packed_text}"
    );
}
