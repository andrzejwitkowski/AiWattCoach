use futures::TryStreamExt;
use mongodb::{bson::doc, Client};

use aiwattcoach::{adapters::mongo::workout_summary::MongoWorkoutSummaryRepository, Settings};

#[tokio::test]
async fn workout_summary_repository_creates_legacy_event_id_index() {
    let settings = Settings::test_defaults();
    let client = Client::with_uri_str(&settings.mongo.uri)
        .await
        .expect("test mongo client should be created");
    let database = format!(
        "aiwattcoach_workout_summary_mongo_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let repository = MongoWorkoutSummaryRepository::new(client.clone(), &database);
    repository.ensure_indexes().await.unwrap();

    let indexes = client
        .database(&database)
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

    client.database(&database).drop().await.unwrap();
}
