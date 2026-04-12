use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{race_calendar::MongoRaceCalendarSource, races::MongoRaceRepository},
    domain::{
        calendar_labels::CalendarLabelSource,
        intervals::DateRange,
        races::{Race, RaceDiscipline, RacePriority, RaceRepository},
    },
    Settings,
};
use futures::TryStreamExt;
use mongodb::{bson::doc, Client};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn race_repository_round_trips_race_and_lists_by_date_range() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoRaceRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_race("race-1", "2026-09-12"))
        .await
        .unwrap();
    repository
        .upsert(sample_race("race-2", "2026-10-01"))
        .await
        .unwrap();

    let found = repository
        .find_by_user_id_and_race_id("user-1", "race-1")
        .await
        .unwrap()
        .expect("expected race");
    assert_eq!(found.name, "Gravel Attack");

    let listed = repository
        .list_by_user_id_and_range(
            "user-1",
            &DateRange {
                oldest: "2026-09-01".to_string(),
                newest: "2026-09-30".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].race_id, "race-1");

    fixture.cleanup().await;
}

#[tokio::test]
async fn race_repository_creates_expected_indexes() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoRaceRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("races")
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
            == Some("races_user_race_unique")
            && index.keys == doc! { "user_id": 1, "race_id": 1 }
    }));
    fixture.cleanup().await;
}

#[tokio::test]
async fn race_repository_exposes_races_as_calendar_labels() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoRaceRepository::new(fixture.client.clone(), &fixture.database);
    let calendar_source = MongoRaceCalendarSource::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_race("race-1", "2026-09-12"))
        .await
        .unwrap();
    repository
        .upsert(sample_race("race-2", "2026-09-13"))
        .await
        .unwrap();

    let labels = calendar_source
        .list_labels(
            "user-1",
            &DateRange {
                oldest: "2026-09-01".to_string(),
                newest: "2026-09-30".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0].label_key, "race:race-1");
    assert_eq!(labels[0].title, "Race Gravel Attack");

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
                panic!("races_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping races_mongo test: {error}");
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
        let database = format!("aiwattcoach_races_mongo_{unique}_{counter}");
        Ok(Self { client, database })
    }

    async fn cleanup(self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}

fn sample_race(race_id: &str, date: &str) -> Race {
    Race {
        race_id: race_id.to_string(),
        user_id: "user-1".to_string(),
        date: date.to_string(),
        name: "Gravel Attack".to_string(),
        distance_meters: 120_000,
        discipline: RaceDiscipline::Gravel,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    }
}
