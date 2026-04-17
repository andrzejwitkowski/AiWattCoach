use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::calendar_entry_views::MongoCalendarEntryViewRepository,
    domain::calendar_view::{
        CalendarEntryKind, CalendarEntrySummary, CalendarEntrySync, CalendarEntryView,
        CalendarEntryViewRepository,
    },
    Settings,
};
use futures::TryStreamExt;
use mongodb::{bson::doc, Client};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn calendar_entry_view_repository_lists_mixed_entries_by_date_range() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCalendarEntryViewRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_entry(
            "planned:1",
            CalendarEntryKind::PlannedWorkout,
            "2026-05-10",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry(
            "completed:1",
            CalendarEntryKind::CompletedWorkout,
            "2026-05-11",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry(
            "race:1",
            CalendarEntryKind::Race,
            "2026-05-12",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry(
            "special:1",
            CalendarEntryKind::SpecialDay,
            "2026-05-13",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry_for_user(
            "planned:2",
            "user-2",
            CalendarEntryKind::PlannedWorkout,
            "2026-05-10",
        ))
        .await
        .unwrap();

    let entries = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].entry_id, "planned:1");
    assert_eq!(entries[1].entry_id, "completed:1");
    assert_eq!(entries[2].entry_id, "race:1");
    assert_eq!(entries[3].entry_id, "special:1");

    fixture.cleanup().await;
}

#[tokio::test]
async fn calendar_entry_view_repository_replaces_all_entries_for_user() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCalendarEntryViewRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_entry(
            "planned:1",
            CalendarEntryKind::PlannedWorkout,
            "2026-05-10",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry_for_user(
            "planned:2",
            "user-2",
            CalendarEntryKind::PlannedWorkout,
            "2026-05-10",
        ))
        .await
        .unwrap();

    repository
        .replace_all_for_user(
            "user-1",
            vec![sample_entry(
                "race:1",
                CalendarEntryKind::Race,
                "2026-05-12",
            )],
        )
        .await
        .unwrap();

    let user_1_entries = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    let user_2_entries = repository
        .list_by_user_id_and_date_range("user-2", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(user_1_entries.len(), 1);
    assert_eq!(user_1_entries[0].entry_id, "race:1");
    assert_eq!(user_2_entries.len(), 1);
    assert_eq!(user_2_entries[0].entry_id, "planned:2");

    fixture.cleanup().await;
}

#[tokio::test]
async fn calendar_entry_view_repository_replace_all_overwrites_stale_user_rows() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCalendarEntryViewRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_entry(
            "planned:1",
            CalendarEntryKind::PlannedWorkout,
            "2026-05-10",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry(
            "race:1",
            CalendarEntryKind::Race,
            "2026-05-12",
        ))
        .await
        .unwrap();

    repository
        .replace_all_for_user(
            "user-1",
            vec![sample_entry(
                "special:1",
                CalendarEntryKind::SpecialDay,
                "2026-05-13",
            )],
        )
        .await
        .unwrap();

    let user_entries = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(user_entries.len(), 1);
    assert_eq!(user_entries[0].entry_id, "special:1");

    fixture.cleanup().await;
}

#[tokio::test]
async fn calendar_entry_view_repository_replaces_only_target_range_and_handles_date_moves() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCalendarEntryViewRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_entry(
            "planned:1",
            CalendarEntryKind::PlannedWorkout,
            "2026-05-10",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry(
            "race:1",
            CalendarEntryKind::Race,
            "2026-05-12",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_entry(
            "special:1",
            CalendarEntryKind::SpecialDay,
            "2026-05-20",
        ))
        .await
        .unwrap();

    repository
        .replace_range_for_user(
            "user-1",
            "2026-05-10",
            "2026-05-12",
            vec![sample_entry(
                "planned:1",
                CalendarEntryKind::PlannedWorkout,
                "2026-05-11",
            )],
        )
        .await
        .unwrap();

    let user_entries = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(user_entries.len(), 2);
    assert!(user_entries
        .iter()
        .any(|entry| entry.entry_id == "planned:1" && entry.date == "2026-05-11"));
    assert!(user_entries
        .iter()
        .any(|entry| entry.entry_id == "special:1"));
    assert!(!user_entries.iter().any(|entry| entry.entry_id == "race:1"));

    fixture.cleanup().await;
}

#[tokio::test]
async fn calendar_entry_view_repository_rejects_replace_range_entries_outside_requested_dates() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCalendarEntryViewRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let error = repository
        .replace_range_for_user(
            "user-1",
            "2026-05-10",
            "2026-05-12",
            vec![sample_entry(
                "planned:1",
                CalendarEntryKind::PlannedWorkout,
                "2026-05-15",
            )],
        )
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("calendar entry date out of range"));

    fixture.cleanup().await;
}

#[tokio::test]
async fn calendar_entry_view_repository_creates_expected_indexes() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoCalendarEntryViewRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("calendar_entry_views")
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
            == Some("calendar_entry_views_user_entry_unique")
            && index.keys == doc! { "user_id": 1, "entry_id": 1 }
    }));
    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("calendar_entry_views_user_date")
            && index.keys == doc! { "user_id": 1, "date": 1 }
    }));
    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("calendar_entry_views_user_kind_date")
            && index.keys == doc! { "user_id": 1, "entry_kind": 1, "date": 1 }
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
                panic!("calendar_entry_views_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping calendar_entry_views_mongo test: {error}");
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
        let database = format!("aiwattcoach_calendar_entry_views_{unique}_{counter}");
        Ok(Self { client, database })
    }

    async fn cleanup(self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}

fn sample_entry(entry_id: &str, entry_kind: CalendarEntryKind, date: &str) -> CalendarEntryView {
    sample_entry_for_user(entry_id, "user-1", entry_kind, date)
}

fn sample_entry_for_user(
    entry_id: &str,
    user_id: &str,
    entry_kind: CalendarEntryKind,
    date: &str,
) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: entry_id.to_string(),
        user_id: user_id.to_string(),
        entry_kind,
        date: date.to_string(),
        start_date_local: Some(format!("{date}T00:00:00")),
        title: format!("Entry {entry_id}"),
        subtitle: Some("subtitle".to_string()),
        description: Some("description".to_string()),
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: Some("- 10m 55%".to_string()),
        planned_workout_id: entry_id
            .starts_with("planned:")
            .then(|| entry_id.replace("planned:", "planned-")),
        completed_workout_id: entry_id
            .starts_with("completed:")
            .then(|| entry_id.replace("completed:", "completed-")),
        race_id: entry_id
            .starts_with("race:")
            .then(|| entry_id.replace("race:", "race-")),
        special_day_id: entry_id
            .starts_with("special:")
            .then(|| entry_id.replace("special:", "special-")),
        race: entry_id.starts_with("race:").then(|| {
            aiwattcoach::domain::calendar_view::CalendarEntryRace {
                distance_meters: 120_000,
                discipline: "gravel".to_string(),
                priority: "B".to_string(),
            }
        }),
        summary: Some(CalendarEntrySummary {
            training_stress_score: Some(82),
            intensity_factor: Some(0.86),
            normalized_power_watts: Some(252),
        }),
        sync: Some(CalendarEntrySync {
            linked_intervals_event_id: Some(41),
            sync_status: Some("synced".to_string()),
        }),
    }
}
