use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{
        completed_workouts::MongoCompletedWorkoutRepository,
        planned_completed_links::MongoPlannedCompletedWorkoutLinkRepository,
        planned_workout_tokens::MongoPlannedWorkoutTokenRepository,
        planned_workouts::MongoPlannedWorkoutRepository, special_days::MongoSpecialDayRepository,
        training_plan_projections::MongoTrainingPlanProjectionRepository,
    },
    domain::{
        completed_workouts::{
            CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
            CompletedWorkoutRepository, CompletedWorkoutSeries, CompletedWorkoutStream,
            CompletedWorkoutZoneTime,
        },
        planned_completed_links::{
            PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkMatchSource,
            PlannedCompletedWorkoutLinkRepository,
        },
        planned_workout_tokens::{PlannedWorkoutToken, PlannedWorkoutTokenRepository},
        planned_workouts::{
            PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutRepository,
            PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
        },
        special_days::{SpecialDay, SpecialDayKind, SpecialDayRepository},
        training_plan::{
            TrainingPlanDay, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
            TrainingPlanSnapshot,
        },
    },
    Settings,
};
use futures::TryStreamExt;
use mongodb::{bson::doc, Client};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn planned_workout_repository_reads_active_projected_days_as_canonical_workouts() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let projection_repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    projection_repository.ensure_indexes().await.unwrap();
    let repository = MongoPlannedWorkoutRepository::new(fixture.client.clone(), &fixture.database);

    let snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    projection_repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot),
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    let workouts = repository
        .list_by_user_id_and_date_range("user-1", "2026-04-07", "2026-04-30")
        .await
        .unwrap();

    assert_eq!(workouts.len(), 13);
    assert_eq!(
        workouts[0].planned_workout_id,
        "training-plan:user-1:workout-1:1700000000:2026-04-07"
    );
    assert_eq!(workouts[0].date, "2026-04-07");
    assert_eq!(workouts[0].user_id, "user-1");
    assert_eq!(workouts[0].workout.lines.len(), 2);

    fixture.cleanup().await;
}

#[tokio::test]
async fn planned_workout_repository_excludes_snapshot_start_date_even_when_it_has_workout() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let projection_repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    projection_repository.ensure_indexes().await.unwrap();
    let repository = MongoPlannedWorkoutRepository::new(fixture.client.clone(), &fixture.database);

    let snapshot = sample_snapshot_with_workout_on_start_date(
        "training-plan:user-1:workout-2:1700000001",
        "2026-04-06",
    );
    projection_repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot),
            "2026-04-06",
            1_700_000_001,
        )
        .await
        .unwrap();

    let workouts = repository
        .list_by_user_id_and_date_range("user-1", "2026-04-06", "2026-04-30")
        .await
        .unwrap();

    assert_eq!(workouts.len(), 13);
    assert!(!workouts.iter().any(|workout| workout.date == "2026-04-06"));

    fixture.cleanup().await;
}

#[tokio::test]
async fn planned_workout_repository_merges_imported_and_projected_workouts() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let projection_repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    projection_repository.ensure_indexes().await.unwrap();
    let repository = MongoPlannedWorkoutRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let snapshot = sample_snapshot("training-plan:user-1:workout-3:1700000002", "2026-04-06");
    projection_repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot),
            "2026-04-06",
            1_700_000_002,
        )
        .await
        .unwrap();
    repository
        .upsert(sample_imported_planned_workout(
            "imported-planned-1",
            "user-1",
            "2026-04-20",
        ))
        .await
        .unwrap();

    let workouts = repository
        .list_by_user_id_and_date_range("user-1", "2026-04-07", "2026-04-30")
        .await
        .unwrap();

    assert!(workouts
        .iter()
        .any(|workout| workout.planned_workout_id == "imported-planned-1"));
    assert!(workouts.iter().any(|workout| {
        workout.planned_workout_id == "training-plan:user-1:workout-3:1700000002:2026-04-07"
    }));

    fixture.cleanup().await;
}

#[tokio::test]
async fn completed_workout_repository_round_trips_canonical_completed_workouts() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCompletedWorkoutRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_completed_workout(
            "completed-2",
            "user-1",
            "2026-05-02T08:00:00",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_completed_workout(
            "completed-1",
            "user-1",
            "2026-05-01T08:00:00",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_completed_workout(
            "completed-3",
            "user-2",
            "2026-05-01T08:00:00",
        ))
        .await
        .unwrap();

    let workouts = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(workouts.len(), 2);
    assert_eq!(workouts[0].completed_workout_id, "completed-1");
    assert_eq!(workouts[0].metrics.training_stress_score, Some(78));
    assert_eq!(workouts[0].details.streams.len(), 1);
    assert_eq!(workouts[1].completed_workout_id, "completed-2");

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("completed_workouts")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("completed_workouts_user_completed_workout_unique")
            && index.keys == doc! { "user_id": 1, "completed_workout_id": 1 }
    }));

    fixture.cleanup().await;
}

#[tokio::test]
async fn special_day_repository_round_trips_and_lists_by_date_range() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoSpecialDayRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_special_day("special-2", "user-1", "2026-05-02"))
        .await
        .unwrap();
    repository
        .upsert(sample_special_day("special-1", "user-1", "2026-05-01"))
        .await
        .unwrap();
    repository
        .upsert(sample_special_day("special-3", "user-2", "2026-05-01"))
        .await
        .unwrap();

    let special_days = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(special_days.len(), 2);
    assert_eq!(special_days[0].special_day_id, "special-1");
    assert_eq!(special_days[1].special_day_id, "special-2");

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("special_days")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("special_days_user_special_day_unique")
            && index.keys == doc! { "user_id": 1, "special_day_id": 1 }
    }));

    fixture.cleanup().await;
}

#[tokio::test]
async fn planned_workout_token_repository_round_trips_and_indexes_tokens() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoPlannedWorkoutTokenRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(PlannedWorkoutToken::new(
            "user-1".to_string(),
            "planned-1".to_string(),
            "PW123ABC".to_string(),
        ))
        .await
        .unwrap();

    let by_planned = repository
        .find_by_planned_workout_id("user-1", "planned-1")
        .await
        .unwrap();
    let by_token = repository
        .find_by_match_token("user-1", "PW123ABC")
        .await
        .unwrap();

    assert_eq!(by_planned, by_token);
    assert_eq!(
        by_planned.map(|token| token.match_token),
        Some("PW123ABC".to_string())
    );

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("planned_workout_tokens")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("planned_workout_tokens_user_planned_workout_unique")
            && index.keys == doc! { "user_id": 1, "planned_workout_id": 1 }
    }));
    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("planned_workout_tokens_user_match_token_unique")
            && index.keys == doc! { "user_id": 1, "match_token": 1 }
    }));

    fixture.cleanup().await;
}

#[tokio::test]
async fn planned_completed_link_repository_round_trips_and_indexes_links() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoPlannedCompletedWorkoutLinkRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-1".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Token,
            1_700_000_123,
        ))
        .await
        .unwrap();

    let by_planned = repository
        .find_by_planned_workout_id("user-1", "planned-1")
        .await
        .unwrap();
    let by_completed = repository
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap();

    assert_eq!(by_planned, by_completed);
    assert_eq!(
        by_planned.map(|link| link.match_source),
        Some(PlannedCompletedWorkoutLinkMatchSource::Token)
    );

    repository
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-2".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            1_700_000_456,
        ))
        .await
        .unwrap();

    assert_eq!(
        repository
            .find_by_planned_workout_id("user-1", "planned-1")
            .await
            .unwrap(),
        None
    );
    let relinked = repository
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap();
    assert_eq!(
        relinked
            .as_ref()
            .map(|link| link.planned_workout_id.as_str()),
        Some("planned-2")
    );
    assert_eq!(
        relinked.map(|link| link.match_source),
        Some(PlannedCompletedWorkoutLinkMatchSource::Explicit)
    );

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("planned_completed_workout_links")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("planned_completed_links_user_planned_unique")
            && index.keys == doc! { "user_id": 1, "planned_workout_id": 1 }
    }));
    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("planned_completed_links_user_completed_unique")
            && index.keys == doc! { "user_id": 1, "completed_workout_id": 1 }
    }));

    fixture.cleanup().await;
}

struct MongoFixture {
    client: Client,
    database: String,
}

async fn mongo_fixture_or_skip() -> Option<MongoFixture> {
    match MongoFixture::new().await {
        Ok(fixture) => Some(fixture),
        Err(error) => {
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("canonical_roots_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping canonical_roots_mongo test: {error}");
            None
        }
    }
}

impl MongoFixture {
    async fn new() -> Result<Self, String> {
        let settings = Settings::test_defaults();
        let mongo_uri = settings.mongo.uri.clone();
        let client = Client::with_uri_str(&settings.mongo.uri)
            .await
            .map_err(|error| {
                format!("failed to create test mongo client for {mongo_uri}: {error}")
            })?;
        tokio::time::timeout(
            Duration::from_secs(1),
            client.database("admin").run_command(doc! { "ping": 1 }),
        )
        .await
        .map_err(|_| format!("timed out connecting to Mongo at {mongo_uri}"))?
        .map_err(|error| format!("failed to connect to Mongo at {mongo_uri}: {error}"))?;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let database = format!("aiwattcoach_canonical_roots_mongo_{unique}_{counter}");
        Ok(Self { client, database })
    }

    async fn cleanup(self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}

fn sample_snapshot(operation_key: &str, start_date: &str) -> TrainingPlanSnapshot {
    let start = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d").unwrap();
    let days = (0..14)
        .map(|offset| {
            let date = start
                .checked_add_signed(chrono::Duration::days(offset))
                .unwrap()
                .format("%Y-%m-%d")
                .to_string();
            TrainingPlanDay {
                date,
                rest_day: offset == 0,
                workout: (offset != 0).then(sample_planned_workout),
            }
        })
        .collect::<Vec<_>>();
    TrainingPlanSnapshot {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: operation_key.to_string(),
        saved_at_epoch_seconds: 1_700_000_000,
        start_date: days.first().unwrap().date.clone(),
        end_date: days.last().unwrap().date.clone(),
        days,
        created_at_epoch_seconds: 1_700_000_000,
    }
}

fn sample_snapshot_with_workout_on_start_date(
    operation_key: &str,
    start_date: &str,
) -> TrainingPlanSnapshot {
    let mut snapshot = sample_snapshot(operation_key, start_date);
    if let Some(first_day) = snapshot.days.first_mut() {
        first_day.rest_day = false;
        first_day.workout = Some(sample_planned_workout());
    }
    snapshot
}

fn sample_projected_days(snapshot: &TrainingPlanSnapshot) -> Vec<TrainingPlanProjectedDay> {
    snapshot
        .days
        .iter()
        .map(|day| TrainingPlanProjectedDay {
            user_id: snapshot.user_id.clone(),
            workout_id: snapshot.workout_id.clone(),
            operation_key: snapshot.operation_key.clone(),
            date: day.date.clone(),
            rest_day: day.rest_day,
            workout: day.workout.clone(),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: snapshot.created_at_epoch_seconds,
            updated_at_epoch_seconds: snapshot.created_at_epoch_seconds,
        })
        .collect()
}

fn sample_planned_workout() -> aiwattcoach::domain::intervals::PlannedWorkout {
    aiwattcoach::domain::intervals::PlannedWorkout {
        lines: vec![
            aiwattcoach::domain::intervals::PlannedWorkoutLine::Text(
                aiwattcoach::domain::intervals::PlannedWorkoutText {
                    text: "AI Threshold".to_string(),
                },
            ),
            aiwattcoach::domain::intervals::PlannedWorkoutLine::Step(
                aiwattcoach::domain::intervals::PlannedWorkoutStep {
                    duration_seconds: 600,
                    kind: aiwattcoach::domain::intervals::PlannedWorkoutStepKind::Steady,
                    target: aiwattcoach::domain::intervals::PlannedWorkoutTarget::PercentFtp {
                        min: 92.0,
                        max: 97.0,
                    },
                },
            ),
        ],
    }
}

fn sample_imported_planned_workout(
    planned_workout_id: &str,
    user_id: &str,
    date: &str,
) -> PlannedWorkout {
    PlannedWorkout::new(
        planned_workout_id.to_string(),
        user_id.to_string(),
        date.to_string(),
        PlannedWorkoutContent {
            lines: vec![
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Imported Threshold".to_string(),
                }),
                PlannedWorkoutLine::Step(PlannedWorkoutStep {
                    duration_seconds: 900,
                    kind: PlannedWorkoutStepKind::Steady,
                    target: PlannedWorkoutTarget::PercentFtp {
                        min: 90.0,
                        max: 95.0,
                    },
                }),
            ],
        },
    )
}

fn sample_completed_workout(
    completed_workout_id: &str,
    user_id: &str,
    start_date_local: &str,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        user_id.to_string(),
        start_date_local.to_string(),
        Some(completed_workout_id.to_string()),
        None,
        Some("Threshold Ride".to_string()),
        Some("Strong day".to_string()),
        Some("Ride".to_string()),
        Some("external-1".to_string()),
        false,
        Some(3600),
        Some(35_000.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(78),
            normalized_power_watts: Some(245),
            intensity_factor: Some(0.83),
            efficiency_factor: None,
            variability_index: Some(1.04),
            average_power_watts: Some(221),
            ftp_watts: Some(295),
            total_work_joules: Some(750),
            calories: Some(900),
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z3".to_string(),
                seconds: 1200,
            }],
            heart_rate_zone_times: vec![600],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    )
}

fn sample_special_day(special_day_id: &str, user_id: &str, date: &str) -> SpecialDay {
    SpecialDay::new(
        special_day_id.to_string(),
        user_id.to_string(),
        date.to_string(),
        SpecialDayKind::Illness,
        Some("Illness".to_string()),
        Some("Recovery day".to_string()),
    )
    .unwrap()
}
