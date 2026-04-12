use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    Client,
};

use aiwattcoach::{
    adapters::mongo::pest_parser_poc_workouts::MongoPestParserPocWorkoutRepository,
    domain::intervals::{
        parse_workout_doc, NoopPestParserPocRepository, PestParserPocDirection,
        PestParserPocOperation, PestParserPocRecordContext, PestParserPocRepositoryPort,
        PestParserPocSource, PestParserPocStatus, PestParserPocWorkoutRecord,
    },
    Settings,
};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn builds_failed_poc_record() {
    let record = PestParserPocWorkoutRecord::failed(
        PestParserPocRecordContext {
            user_id: "user-1".to_string(),
            source: PestParserPocSource {
                direction: PestParserPocDirection::Inbound,
                operation: PestParserPocOperation::ListEvents,
            },
            source_ref: Some("event-7".to_string()),
            source_text: "- 10m ???".to_string(),
            parser_version: "v1".to_string(),
            parsed_at_epoch_seconds: 123,
        },
        "invalid target".to_string(),
        "syntax".to_string(),
        parse_workout_doc(Some("- 10m ???"), None),
    );

    assert_eq!(record.status, PestParserPocStatus::Failed);
    assert_eq!(record.error_kind.as_deref(), Some("syntax"));
    assert!(record.legacy_projection.is_some());
}

#[test]
fn builds_parsed_poc_record() {
    let parsed = parse_workout_doc(Some("- 10m 95%"), Some(300));

    let record = PestParserPocWorkoutRecord::parsed(
        PestParserPocRecordContext {
            user_id: "user-1".to_string(),
            source: PestParserPocSource {
                direction: PestParserPocDirection::Outbound,
                operation: PestParserPocOperation::UpdateEvent,
            },
            source_ref: None,
            source_text: "- 10m 95%".to_string(),
            parser_version: "v1".to_string(),
            parsed_at_epoch_seconds: 123,
        },
        "10m 95%".to_string(),
        parsed,
    );

    assert_eq!(record.status, PestParserPocStatus::Parsed);
    assert_eq!(record.normalized_workout.as_deref(), Some("10m 95%"));
}

#[test]
fn noop_poc_repository_accepts_records() {
    let repository = NoopPestParserPocRepository;
    let result =
        futures::executor::block_on(repository.insert(PestParserPocWorkoutRecord::failed(
            PestParserPocRecordContext {
                user_id: "user-1".to_string(),
                source: PestParserPocSource {
                    direction: PestParserPocDirection::Inbound,
                    operation: PestParserPocOperation::GetEvent,
                },
                source_ref: None,
                source_text: "bad".to_string(),
                parser_version: "v1".to_string(),
                parsed_at_epoch_seconds: 1,
            },
            "err".to_string(),
            "syntax".to_string(),
            parse_workout_doc(Some("bad"), None),
        )));

    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn pest_parser_poc_repository_inserts_failed_record_document() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoPestParserPocWorkoutRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .insert(PestParserPocWorkoutRecord::failed(
            PestParserPocRecordContext {
                user_id: "user-1".to_string(),
                source: PestParserPocSource {
                    direction: PestParserPocDirection::Inbound,
                    operation: PestParserPocOperation::ListEvents,
                },
                source_ref: Some("event-1".to_string()),
                source_text: "- 10m ???".to_string(),
                parser_version: "v1".to_string(),
                parsed_at_epoch_seconds: 101,
            },
            "invalid syntax".to_string(),
            "syntax".to_string(),
            parse_workout_doc(Some("- 10m ???"), None),
        ))
        .await
        .unwrap();

    let document = fixture
        .collection()
        .find_one(doc! {})
        .await
        .unwrap()
        .unwrap();
    assert_eq!(document.get_str("user_id").unwrap(), "user-1");
    assert_eq!(document.get_str("status").unwrap(), "failed");
    assert_eq!(document.get_str("error_kind").unwrap(), "syntax");

    fixture.cleanup().await;
}

#[tokio::test]
async fn pest_parser_poc_repository_creates_expected_indexes() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoPestParserPocWorkoutRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let indexes = fixture
        .collection()
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
            == Some("pest_parser_poc_workout_user_time")
            && index.keys == doc! { "user_id": 1, "parsed_at_epoch_seconds": -1 }
    }));
    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("pest_parser_poc_workout_status_time")
            && index.keys == doc! { "status": 1, "parsed_at_epoch_seconds": -1 }
    }));
    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("pest_parser_poc_workout_operation_source_time")
            && index.keys == doc! { "operation": 1, "source_ref": 1, "parsed_at_epoch_seconds": -1 }
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
                panic!("intervals_pest_parser_poc test requires Mongo in CI: {error}");
            }
            eprintln!("skipping intervals_pest_parser_poc test: {error}");
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
        let database = format!("aiwattcoach_pest_parser_poc_{unique}_{counter}");

        Ok(Self { client, database })
    }

    fn collection(&self) -> mongodb::Collection<Document> {
        self.client
            .database(&self.database)
            .collection("pest_parser_poc_workout")
    }

    async fn cleanup(&self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}
