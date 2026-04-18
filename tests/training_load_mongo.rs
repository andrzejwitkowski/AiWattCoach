use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{
        ftp_history::MongoFtpHistoryRepository,
        training_load_daily_snapshots::MongoTrainingLoadDailySnapshotRepository,
    },
    domain::training_load::{
        FtpHistoryEntry, FtpHistoryRepository, FtpSource, TrainingLoadDailySnapshot,
        TrainingLoadDailySnapshotRepository, TrainingLoadSnapshotRange,
    },
    Settings,
};
use mongodb::{bson::doc, Client};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn ftp_history_repository_upserts_and_resolves_effective_entry() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoFtpHistoryRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_ftp_entry("user-1", "2026-04-01", 280))
        .await
        .unwrap();
    repository
        .upsert(sample_ftp_entry("user-1", "2026-04-10", 290))
        .await
        .unwrap();
    repository
        .upsert(sample_ftp_entry("user-1", "2026-04-10", 295))
        .await
        .unwrap();

    let history = repository.list_by_user_id("user-1").await.unwrap();
    let effective = repository
        .find_effective_for_date("user-1", "2026-04-17")
        .await
        .unwrap();

    assert_eq!(history.len(), 2);
    assert_eq!(history[0].ftp_watts, 280);
    assert_eq!(history[1].ftp_watts, 295);
    assert_eq!(effective.map(|entry| entry.ftp_watts), Some(295));

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_load_daily_snapshot_repository_replaces_days_and_deletes_from_date() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingLoadDailySnapshotRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .upsert(sample_snapshot("user-1", "2026-04-01", Some(50), Some(280)))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot("user-1", "2026-04-02", Some(60), Some(280)))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot("user-1", "2026-04-02", Some(75), Some(285)))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot("user-1", "2026-04-03", Some(80), Some(285)))
        .await
        .unwrap();

    let before_delete = repository
        .list_by_user_id_and_range(
            "user-1",
            &TrainingLoadSnapshotRange {
                oldest: "2026-04-01".to_string(),
                newest: "2026-04-30".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(before_delete.len(), 3);
    assert_eq!(before_delete[1].daily_tss, Some(75));
    assert_eq!(before_delete[1].ftp_effective_watts, Some(285));

    repository
        .delete_by_user_id_from_date("user-1", "2026-04-02")
        .await
        .unwrap();

    let after_delete = repository
        .list_by_user_id_and_range(
            "user-1",
            &TrainingLoadSnapshotRange {
                oldest: "2026-04-01".to_string(),
                newest: "2026-04-30".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(after_delete.len(), 1);
    assert_eq!(after_delete[0].date, "2026-04-01");

    fixture.cleanup().await;
}

#[test]
fn redact_uri_credentials_hides_mongo_userinfo() {
    assert_eq!(
        redact_uri_credentials("mongodb://user:secret@example.com:27017/aiwattcoach"),
        "mongodb://<redacted>@example.com:27017/aiwattcoach"
    );
    assert_eq!(
        redact_uri_credentials("mongodb://localhost:27017/aiwattcoach"),
        "mongodb://localhost:27017/aiwattcoach"
    );
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
                panic!("training_load_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping training_load_mongo test: {error}");
            None
        }
    }
}

impl MongoFixture {
    async fn new() -> Result<Self, String> {
        let settings = Settings::test_defaults();
        let mongo_uri = settings.mongo.uri.clone();
        let mongo_uri_for_log = redact_uri_credentials(&mongo_uri);
        let client = Client::with_uri_str(&settings.mongo.uri)
            .await
            .map_err(|error| {
                format!("failed to create test mongo client for {mongo_uri_for_log}: {error}")
            })?;
        tokio::time::timeout(
            Duration::from_secs(1),
            client.database("admin").run_command(doc! { "ping": 1 }),
        )
        .await
        .map_err(|_| format!("timed out connecting to Mongo at {mongo_uri_for_log}"))?
        .map_err(|error| format!("failed to connect to Mongo at {mongo_uri_for_log}: {error}"))?;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let database = format!("aiwattcoach_training_load_mongo_{unique}_{counter}");
        Ok(Self { client, database })
    }

    async fn cleanup(self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}

fn redact_uri_credentials(uri: &str) -> String {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return "<redacted-mongo-uri>".to_string();
    };

    if let Some((_, host_and_path)) = rest.split_once('@') {
        format!("{scheme}://<redacted>@{host_and_path}")
    } else {
        uri.to_string()
    }
}

fn sample_ftp_entry(user_id: &str, effective_from_date: &str, ftp_watts: i32) -> FtpHistoryEntry {
    FtpHistoryEntry {
        user_id: user_id.to_string(),
        effective_from_date: effective_from_date.to_string(),
        ftp_watts,
        source: FtpSource::Settings,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

fn sample_snapshot(
    user_id: &str,
    date: &str,
    daily_tss: Option<i32>,
    ftp_effective_watts: Option<i32>,
) -> TrainingLoadDailySnapshot {
    TrainingLoadDailySnapshot {
        user_id: user_id.to_string(),
        date: date.to_string(),
        daily_tss,
        rolling_tss_7d: Some(42.0),
        rolling_tss_28d: Some(38.0),
        ctl: Some(50.0),
        atl: Some(60.0),
        tsb: Some(-10.0),
        average_if_28d: Some(0.85),
        average_ef_28d: Some(1.35),
        ftp_effective_watts,
        ftp_source: ftp_effective_watts.map(|_| FtpSource::Settings),
        recomputed_at_epoch_seconds: 1_700_000_100,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_100,
    }
}
