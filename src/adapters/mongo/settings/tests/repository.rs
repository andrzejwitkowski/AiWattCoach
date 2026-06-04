use futures::TryStreamExt;
use mongodb::bson::{doc, Document};

use super::super::{
    documents::{default_availability_document, SettingsDocument, WahooDocument},
    mapping::map_domain_availability_to_document,
    repository::MongoUserSettingsRepository,
};
use super::support::{
    build_settings_document, test_mongo_client_or_skip, unique_test_database_name,
};
use crate::domain::settings::{
    AvailabilityDay, AvailabilitySettings, UserSettingsRepository, Weekday,
};

#[tokio::test]
async fn update_availability_updates_only_target_user_document() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("user-settings-availability");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<SettingsDocument>("user_settings");

    let user_1_id = "user-availability-target";
    let user_2_id = "user-availability-untouched";

    collection
        .insert_many([
            build_settings_document(user_1_id, 10),
            build_settings_document(user_2_id, 20),
        ])
        .await
        .unwrap();

    let availability = AvailabilitySettings {
        configured: true,
        days: vec![
            AvailabilityDay {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Tue,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Wed,
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDay {
                weekday: Weekday::Thu,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Fri,
                available: true,
                max_duration_minutes: Some(120),
            },
            AvailabilityDay {
                weekday: Weekday::Sat,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
        ],
    };
    let updated_at = 123_456;

    repository
        .update_availability(user_1_id, availability.clone(), updated_at)
        .await
        .unwrap();

    let updated = collection
        .find_one(doc! { "user_id": user_1_id })
        .await
        .unwrap()
        .unwrap();
    let untouched = collection
        .find_one(doc! { "user_id": user_2_id })
        .await
        .unwrap()
        .unwrap();

    let expected_availability = map_domain_availability_to_document(&availability);

    assert_eq!(
        updated.availability.configured,
        expected_availability.configured
    );
    assert_eq!(
        updated.availability.days.len(),
        expected_availability.days.len()
    );
    assert_eq!(
        updated.availability.days[0].weekday,
        expected_availability.days[0].weekday
    );
    assert_eq!(
        updated.availability.days[0].available,
        expected_availability.days[0].available
    );
    assert_eq!(
        updated.availability.days[0].max_duration_minutes,
        expected_availability.days[0].max_duration_minutes
    );
    assert_eq!(
        updated.availability.days[2].weekday,
        expected_availability.days[2].weekday
    );
    assert_eq!(
        updated.availability.days[2].available,
        expected_availability.days[2].available
    );
    assert_eq!(
        updated.availability.days[2].max_duration_minutes,
        expected_availability.days[2].max_duration_minutes
    );
    assert_eq!(updated.updated_at_epoch_seconds, Some(updated_at));

    let default_availability = default_availability_document();

    assert_eq!(untouched.user_id, user_2_id);
    assert_eq!(untouched.updated_at_epoch_seconds, Some(20));
    assert_eq!(
        untouched.availability.configured,
        default_availability.configured
    );
    assert_eq!(
        untouched.availability.days.len(),
        default_availability.days.len()
    );
    assert!(untouched.availability.days.iter().all(|day| !day.available));
    assert_eq!(untouched.availability.days[0].weekday, "mon");
    assert_eq!(untouched.availability.days[6].weekday, "sun");

    client.database(&database_name).drop().await.unwrap();
}

#[tokio::test]
async fn ensure_indexes_creates_unique_wahoo_user_id_index() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("mongo-settings-wahoo-user-id-index");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);

    repository.ensure_indexes().await.unwrap();

    let index_names = client
        .database(&database_name)
        .collection::<mongodb::bson::Document>("user_settings")
        .list_index_names()
        .await
        .unwrap();
    assert!(index_names.contains(&"user_settings_wahoo_user_id_unique".to_string()));

    let indexes = client
        .database(&database_name)
        .collection::<mongodb::bson::Document>("user_settings")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let wahoo_index = indexes
        .iter()
        .find(|idx| {
            idx.options.as_ref().and_then(|opts| opts.name.as_deref())
                == Some("user_settings_wahoo_user_id_unique")
        })
        .expect("wahoo user_id index should exist");
    assert_eq!(wahoo_index.keys.get_i32("wahoo.user_id").unwrap(), 1);
    let unique = wahoo_index
        .options
        .as_ref()
        .and_then(|opts| opts.unique)
        .unwrap_or(false);
    assert!(unique, "wahoo user_id index should be unique");
    assert!(
        wahoo_index
            .options
            .as_ref()
            .and_then(|opts| opts.partial_filter_expression.as_ref())
            .is_some(),
        "wahoo user_id index should have partialFilterExpression"
    );

    client.database(&database_name).drop().await.unwrap();
}

#[tokio::test]
async fn find_by_wahoo_user_id_rejects_duplicate_mappings() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("mongo-settings-wahoo-user-id-duplicates");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<SettingsDocument>("user_settings");

    collection
        .insert_many([
            SettingsDocument {
                wahoo: WahooDocument {
                    user_id: Some(60_462),
                    refresh_token: Some("refresh-1".to_string()),
                    connected: true,
                    ..WahooDocument::default()
                },
                ..build_settings_document("user-1", 10)
            },
            SettingsDocument {
                wahoo: WahooDocument {
                    user_id: Some(60_462),
                    refresh_token: Some("refresh-2".to_string()),
                    connected: true,
                    ..WahooDocument::default()
                },
                ..build_settings_document("user-2", 20)
            },
        ])
        .await
        .unwrap();

    let error = repository
        .find_by_wahoo_user_id(60_462)
        .await
        .expect_err("duplicate wahoo user id should be rejected");
    assert!(error
        .to_string()
        .contains("multiple users are mapped to Wahoo user id 60462"));

    client.database(&database_name).drop().await.unwrap();
}

#[tokio::test]
async fn backfill_wahoo_user_id_updates_only_wahoo_fields() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("mongo-settings-wahoo-user-id-backfill");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<Document>("user_settings");

    collection
        .insert_one(doc! {
            "user_id": "user-1",
            "ai_agents": {},
            "intervals": {
                "api_key": "api-key",
                "athlete_id": "athlete-1",
                "connected": true,
                "updated_at_epoch_seconds": 123,
            },
            "wahoo": {
                "refresh_token": "refresh-token",
                "connected": true,
            },
            "options": {},
            "availability": { "configured": false, "days": [] },
            "cycling": {},
            "created_at_epoch_seconds": 1,
            "updated_at_epoch_seconds": 2,
        })
        .await
        .unwrap();

    repository
        .backfill_wahoo_user_id("user-1", 60_462, 1_700_000_000)
        .await
        .unwrap();

    let updated = collection
        .find_one(doc! { "user_id": "user-1" })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.get("updated_at_epoch_seconds").unwrap().as_i32(),
        Some(2)
    );
    assert_eq!(
        updated
            .get_document("intervals")
            .unwrap()
            .get("updated_at_epoch_seconds")
            .unwrap()
            .as_i32(),
        Some(123)
    );
    assert_eq!(
        updated
            .get_document("wahoo")
            .unwrap()
            .get_i64("user_id")
            .unwrap(),
        60_462
    );
    assert_eq!(
        updated
            .get_document("wahoo")
            .unwrap()
            .get_i64("updated_at_epoch_seconds")
            .unwrap(),
        1_700_000_000
    );

    client.database(&database_name).drop().await.unwrap();
}
