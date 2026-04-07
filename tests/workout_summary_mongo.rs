use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use futures::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId},
    Client,
};

use aiwattcoach::{
    adapters::mongo::workout_summary::MongoWorkoutSummaryRepository,
    domain::workout_summary::{WorkoutSummary, WorkoutSummaryRepository},
    Settings,
};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn workout_summary_repository_prefers_current_workout_id_over_legacy_event_id() {
    let fixture = MongoFixture::new().await;
    let repository = MongoWorkoutSummaryRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    fixture
        .collection()
        .insert_many([
            doc! {
                "_id": ObjectId::new(),
                "summary_id": "summary-current",
                "user_id": "user-1",
                "workout_id": "workout-1",
                "rpe": 7,
                "messages": [],
                "saved_at_epoch_seconds": mongodb::bson::Bson::Null,
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 10,
            },
            doc! {
                "_id": ObjectId::new(),
                "summary_id": "summary-legacy",
                "user_id": "user-1",
                "event_id": "workout-1",
                "rpe": 3,
                "messages": [],
                "saved_at_epoch_seconds": mongodb::bson::Bson::Null,
                "created_at_epoch_seconds": 2,
                "updated_at_epoch_seconds": 20,
            },
        ])
        .await
        .unwrap();

    let found = repository
        .find_by_user_id_and_workout_id("user-1", "workout-1")
        .await
        .unwrap()
        .expect("expected summary");

    assert_eq!(found.id, "summary-current");
    assert_eq!(found.workout_id, "workout-1");

    repository
        .update_rpe("user-1", "workout-1", 9, 30)
        .await
        .unwrap();

    let documents = fixture
        .collection()
        .find(doc! { "user_id": "user-1" })
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(documents.len(), 2);

    let current = documents
        .iter()
        .find(|document| document.get_str("summary_id").unwrap() == "summary-current")
        .unwrap();
    assert_eq!(current.get_i32("rpe").unwrap(), 9);

    let legacy = documents
        .iter()
        .find(|document| document.get_str("summary_id").unwrap() == "summary-legacy")
        .unwrap();
    assert_eq!(legacy.get_i32("rpe").unwrap(), 3);
}

#[tokio::test]
async fn workout_summary_repository_list_uses_legacy_fallback_when_current_match_is_absent() {
    let fixture = MongoFixture::new().await;
    let repository = MongoWorkoutSummaryRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    repository
        .create(sample_summary(
            "summary-current",
            "user-1",
            "workout-1",
            Some(6),
            10,
        ))
        .await
        .unwrap();

    fixture
        .collection()
        .insert_one(doc! {
            "_id": ObjectId::new(),
            "summary_id": "summary-fallback",
            "user_id": "user-1",
            "event_id": "workout-2",
            "rpe": 5,
            "messages": [],
            "saved_at_epoch_seconds": mongodb::bson::Bson::Null,
            "created_at_epoch_seconds": 3,
            "updated_at_epoch_seconds": 30,
        })
        .await
        .unwrap();

    let summaries = repository
        .find_by_user_id_and_workout_ids(
            "user-1",
            vec!["workout-1".to_string(), "workout-2".to_string()],
        )
        .await
        .unwrap();

    assert_eq!(summaries.len(), 2);
    assert!(summaries
        .iter()
        .any(|summary| summary.id == "summary-current"));
    assert!(summaries
        .iter()
        .any(|summary| summary.id == "summary-fallback"));
}

#[tokio::test]
async fn workout_summary_repository_creates_legacy_event_id_index() {
    let fixture = MongoFixture::new().await;
    let repository = MongoWorkoutSummaryRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("workout_summaries")
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
            == Some("workout_summaries_user_event")
            && index.keys == doc! { "user_id": 1, "event_id": 1 }
    }));
}

struct MongoFixture {
    client: Client,
    database: String,
}

impl MongoFixture {
    async fn new() -> Self {
        let settings = Settings::test_defaults();
        let client = Client::with_uri_str(&settings.mongo.uri)
            .await
            .expect("test mongo client should be created");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let database = format!("aiwattcoach_workout_summary_mongo_{unique}_{counter}");
        Self { client, database }
    }

    fn collection(&self) -> mongodb::Collection<mongodb::bson::Document> {
        self.client
            .database(&self.database)
            .collection("workout_summaries")
    }
}

impl Drop for MongoFixture {
    fn drop(&mut self) {
        let client = self.client.clone();
        let database = self.database.clone();
        tokio::spawn(async move {
            let _ = client.database(&database).drop().await;
        });
    }
}

fn sample_summary(
    id: &str,
    user_id: &str,
    workout_id: &str,
    rpe: Option<u8>,
    updated_at_epoch_seconds: i64,
) -> WorkoutSummary {
    WorkoutSummary {
        id: id.to_string(),
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        rpe,
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds,
    }
}
