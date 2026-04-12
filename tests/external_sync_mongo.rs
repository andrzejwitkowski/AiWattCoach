use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{
        external_observations::MongoExternalObservationRepository,
        external_sync_states::MongoExternalSyncStateRepository,
        provider_poll_states::MongoProviderPollStateRepository,
    },
    domain::external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservation,
        ExternalObservationRepository, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository, ProviderPollState, ProviderPollStateRepository,
        ProviderPollStream,
    },
    Settings,
};
use futures::TryStreamExt;
use mongodb::{bson::doc, Client};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn external_observation_repository_round_trips_by_provider_and_external_id() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoExternalObservationRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let observation = ExternalObservation::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ExternalObjectKind::CompletedWorkout,
        "remote-1".to_string(),
        CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            "completed-1".to_string(),
        ),
        Some("hash-1".to_string()),
        1_700_000_000,
    );
    repository.upsert(observation.clone()).await.unwrap();

    let found = repository
        .find_by_provider_and_external_id("user-1", ExternalProvider::Intervals, "remote-1")
        .await
        .unwrap();

    assert_eq!(found, Some(observation));

    fixture.cleanup().await;
}

#[tokio::test]
async fn external_sync_state_repository_round_trips_by_provider_and_canonical_entity() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoExternalSyncStateRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let canonical = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        canonical.clone(),
    )
    .mark_synced("44".to_string(), "hash-1".to_string(), 1_700_000_000);
    repository.upsert(state.clone()).await.unwrap();

    let found = repository
        .find_by_provider_and_canonical_entity("user-1", ExternalProvider::Intervals, &canonical)
        .await
        .unwrap();

    assert_eq!(found, Some(state));

    repository
        .delete_by_provider_and_canonical_entity("user-1", ExternalProvider::Intervals, &canonical)
        .await
        .unwrap();
    let deleted = repository
        .find_by_provider_and_canonical_entity("user-1", ExternalProvider::Intervals, &canonical)
        .await
        .unwrap();
    assert_eq!(deleted, None);

    fixture.cleanup().await;
}

#[tokio::test]
async fn provider_poll_state_repository_lists_due_items_and_round_trips_by_stream() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoProviderPollStateRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let due = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_700_000_000,
    );
    let future = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::CompletedWorkouts,
        1_700_000_100,
    );
    repository.upsert(due.clone()).await.unwrap();
    repository.upsert(future).await.unwrap();

    let listed = repository.list_due(1_700_000_000).await.unwrap();
    assert_eq!(listed, vec![due.clone()]);

    let found = repository
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap();
    assert_eq!(found, Some(due));

    fixture.cleanup().await;
}

#[tokio::test]
async fn external_sync_repositories_create_expected_indexes() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };

    let observations =
        MongoExternalObservationRepository::new(fixture.client.clone(), &fixture.database);
    let sync_states =
        MongoExternalSyncStateRepository::new(fixture.client.clone(), &fixture.database);
    let poll_states =
        MongoProviderPollStateRepository::new(fixture.client.clone(), &fixture.database);
    observations.ensure_indexes().await.unwrap();
    sync_states.ensure_indexes().await.unwrap();
    poll_states.ensure_indexes().await.unwrap();

    let observations_indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("external_observations")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert!(observations_indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("external_observations_user_provider_external_unique")
            && index.keys == doc! { "user_id": 1, "provider": 1, "external_id": 1 }
    }));

    let sync_state_indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("external_sync_states")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert!(sync_state_indexes.iter().any(|index| {
        index.options.as_ref().and_then(|options| options.name.as_deref())
            == Some("external_sync_states_user_provider_entity_unique")
            && index.keys
                == doc! { "user_id": 1, "provider": 1, "canonical_entity_kind": 1, "canonical_entity_id": 1 }
    }));

    let poll_state_indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("provider_poll_states")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert!(poll_state_indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("provider_poll_states_user_provider_stream_unique")
            && index.keys == doc! { "user_id": 1, "provider": 1, "stream": 1 }
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
                panic!("external_sync_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping external_sync_mongo test: {error}");
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
        let database = format!("aiwattcoach_external_sync_mongo_{unique}_{counter}");
        Ok(Self { client, database })
    }

    async fn cleanup(self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}
