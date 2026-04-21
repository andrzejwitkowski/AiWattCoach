use std::{
    sync::atomic::{AtomicU64, Ordering},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    Client,
};

use aiwattcoach::{
    adapters::mongo::tasks::MongoTaskRepository,
    domain::task_scheduler::{NewTask, RetryStrategy, ScheduledTask, TaskRepository},
    Settings,
};
use serde_json::json;

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_MONGO_CLIENT: OnceLock<Client> = OnceLock::new();

#[tokio::test]
async fn mongo_task_repository_dedupes_per_user_and_creates_compound_unique_index() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoTaskRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let first = repository
        .enqueue_if_absent(sample_task("task-1", "user-1", "dedupe-1", 100))
        .await
        .expect("first enqueue should succeed");
    let duplicate_same_user = repository
        .enqueue_if_absent(sample_task("task-2", "user-1", "dedupe-1", 100))
        .await
        .expect("same-user duplicate enqueue should succeed");
    let different_user = repository
        .enqueue_if_absent(sample_task("task-3", "user-2", "dedupe-1", 100))
        .await
        .expect("different-user enqueue should succeed");

    assert!(first.created);
    assert!(!duplicate_same_user.created);
    assert_eq!(duplicate_same_user.task.id, "task-1");
    assert!(different_user.created);
    assert_eq!(different_user.task.id, "task-3");

    let documents = fixture
        .collection()
        .find(doc! {})
        .await
        .unwrap()
        .try_collect::<Vec<Document>>()
        .await
        .unwrap();
    assert_eq!(documents.len(), 2);

    let indexes = fixture.index_documents().await;
    assert!(indexes.iter().any(|index| {
        index.keys == doc! { "user_id": 1, "dedupe_key": 1 }
            && index.options.as_ref().and_then(|options| options.unique) == Some(true)
            && index
                .options
                .as_ref()
                .and_then(|options| options.name.as_deref())
                == Some("tasks_dedupe_key_unique")
    }));

    fixture.cleanup().await;
}

fn sample_task(id: &str, user_id: &str, dedupe_key: &str, now_epoch_seconds: i64) -> ScheduledTask {
    ScheduledTask::new(
        NewTask {
            id: id.to_string(),
            user_id: user_id.to_string(),
            task_type: "summary".to_string(),
            payload: json!({ "task": id }),
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 3,
                delay_seconds: 30,
            },
            dedupe_key: dedupe_key.to_string(),
            execution_timeout_seconds: 30,
            leader_only: false,
        },
        now_epoch_seconds,
    )
    .expect("task fixture should be valid")
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
                panic!("task_scheduler_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping task_scheduler_mongo test: {error}");
            None
        }
    }
}

impl MongoFixture {
    async fn new() -> Result<Self, String> {
        let settings = Settings::test_defaults();
        let mongo_uri = settings.mongo.uri.clone();
        let client = if let Some(client) = TEST_MONGO_CLIENT.get() {
            client.clone()
        } else {
            let client = Client::with_uri_str(&settings.mongo.uri)
                .await
                .map_err(|error| {
                    format!("failed to create test mongo client for {mongo_uri}: {error}")
                })?;
            let _ = TEST_MONGO_CLIENT.set(client.clone());
            client
        };
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|error| format!("failed to ping test mongo at {mongo_uri}: {error}"))?;

        Ok(Self {
            client,
            database: unique_test_database_name("task-scheduler-mongo"),
        })
    }

    fn collection(&self) -> mongodb::Collection<Document> {
        self.client.database(&self.database).collection("tasks")
    }

    async fn index_documents(&self) -> Vec<mongodb::IndexModel> {
        self.collection()
            .list_indexes()
            .await
            .unwrap()
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
    }

    async fn cleanup(&self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}

fn unique_test_database_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{nanos}-{counter}")
}
