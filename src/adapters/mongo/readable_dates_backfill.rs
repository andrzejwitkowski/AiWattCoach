use std::{error::Error, fmt};

use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson, DateTime, Document},
    Collection,
};

use super::time::epoch_seconds_to_bson_datetime_with_field;

#[derive(Clone, Copy)]
struct RootTimestampField {
    epoch_path: &'static str,
    datetime_path: &'static str,
}

#[derive(Clone, Copy)]
struct ArrayTimestampField {
    array_path: &'static str,
    epoch_field: &'static str,
    datetime_field: &'static str,
}

#[derive(Clone, Copy)]
struct CollectionBackfillSpec {
    collection_name: &'static str,
    root_fields: &'static [RootTimestampField],
    array_fields: &'static [ArrayTimestampField],
}

#[derive(Debug)]
struct ReadableDatesBackfillError(String);

impl fmt::Display for ReadableDatesBackfillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ReadableDatesBackfillError {}

const COLLECTION_SPECS: &[CollectionBackfillSpec] = &[
    CollectionBackfillSpec {
        collection_name: "user_settings",
        root_fields: &[
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
            RootTimestampField {
                epoch_path: "intervals.updated_at_epoch_seconds",
                datetime_path: "intervals.updated_at",
            },
            RootTimestampField {
                epoch_path: "wahoo.expires_at_epoch_seconds",
                datetime_path: "wahoo.expires_at",
            },
            RootTimestampField {
                epoch_path: "wahoo.updated_at_epoch_seconds",
                datetime_path: "wahoo.updated_at",
            },
            RootTimestampField {
                epoch_path: "cycling.last_zone_update_epoch_seconds",
                datetime_path: "cycling.last_zone_update_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "workout_summaries",
        root_fields: &[
            RootTimestampField {
                epoch_path: "saved_at_epoch_seconds",
                datetime_path: "saved_at",
            },
            RootTimestampField {
                epoch_path: "workout_recap_generated_at_epoch_seconds",
                datetime_path: "workout_recap_generated_at",
            },
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
        ],
        array_fields: &[ArrayTimestampField {
            array_path: "messages",
            epoch_field: "created_at_epoch_seconds",
            datetime_field: "created_at",
        }],
    },
    CollectionBackfillSpec {
        collection_name: "athlete_summary",
        root_fields: &[
            RootTimestampField {
                epoch_path: "generated_at_epoch_seconds",
                datetime_path: "generated_at",
            },
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "planned_workout_syncs",
        root_fields: &[
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
            RootTimestampField {
                epoch_path: "last_synced_at_epoch_seconds",
                datetime_path: "last_synced_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "planned_workout_wahoo_syncs",
        root_fields: &[
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
            RootTimestampField {
                epoch_path: "last_synced_at_epoch_seconds",
                datetime_path: "last_synced_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "planned_completed_workout_links",
        root_fields: &[RootTimestampField {
            epoch_path: "matched_at_epoch_seconds",
            datetime_path: "matched_at",
        }],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "external_observations",
        root_fields: &[RootTimestampField {
            epoch_path: "observed_at_epoch_seconds",
            datetime_path: "observed_at",
        }],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "external_sync_states",
        root_fields: &[
            RootTimestampField {
                epoch_path: "last_synced_at_epoch_seconds",
                datetime_path: "last_synced_at",
            },
            RootTimestampField {
                epoch_path: "last_seen_remote_at_epoch_seconds",
                datetime_path: "last_seen_remote_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "wahoo_fit_files",
        root_fields: &[
            RootTimestampField {
                epoch_path: "downloaded_at_epoch_seconds",
                datetime_path: "downloaded_at",
            },
            RootTimestampField {
                epoch_path: "stored_at_epoch_seconds",
                datetime_path: "stored_at",
            },
            RootTimestampField {
                epoch_path: "parsed_at_epoch_seconds",
                datetime_path: "parsed_at",
            },
            RootTimestampField {
                epoch_path: "enriched_at_epoch_seconds",
                datetime_path: "enriched_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "training_plan_generation_operations",
        root_fields: &[
            RootTimestampField {
                epoch_path: "saved_at_epoch_seconds",
                datetime_path: "saved_at",
            },
            RootTimestampField {
                epoch_path: "workout_recap_generated_at_epoch_seconds",
                datetime_path: "workout_recap_generated_at",
            },
            RootTimestampField {
                epoch_path: "projection_persisted_at_epoch_seconds",
                datetime_path: "projection_persisted_at",
            },
            RootTimestampField {
                epoch_path: "started_at_epoch_seconds",
                datetime_path: "started_at",
            },
            RootTimestampField {
                epoch_path: "last_attempt_at_epoch_seconds",
                datetime_path: "last_attempt_at",
            },
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
        ],
        array_fields: &[ArrayTimestampField {
            array_path: "attempts",
            epoch_field: "recorded_at_epoch_seconds",
            datetime_field: "recorded_at",
        }],
    },
    CollectionBackfillSpec {
        collection_name: "athlete_summary_generation_operations",
        root_fields: &[
            RootTimestampField {
                epoch_path: "started_at_epoch_seconds",
                datetime_path: "started_at",
            },
            RootTimestampField {
                epoch_path: "last_attempt_at_epoch_seconds",
                datetime_path: "last_attempt_at",
            },
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "llm_reply_operations",
        root_fields: &[
            RootTimestampField {
                epoch_path: "started_at_epoch_seconds",
                datetime_path: "started_at",
            },
            RootTimestampField {
                epoch_path: "last_attempt_at_epoch_seconds",
                datetime_path: "last_attempt_at",
            },
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "provider_poll_states",
        root_fields: &[
            RootTimestampField {
                epoch_path: "next_due_at_epoch_seconds",
                datetime_path: "next_due_at",
            },
            RootTimestampField {
                epoch_path: "last_attempted_at_epoch_seconds",
                datetime_path: "last_attempted_at",
            },
            RootTimestampField {
                epoch_path: "last_successful_at_epoch_seconds",
                datetime_path: "last_successful_at",
            },
            RootTimestampField {
                epoch_path: "backoff_until_epoch_seconds",
                datetime_path: "backoff_until_at",
            },
        ],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "task_workers",
        root_fields: &[RootTimestampField {
            epoch_path: "last_heartbeat_at_epoch_seconds",
            datetime_path: "last_heartbeat_at",
        }],
        array_fields: &[],
    },
    CollectionBackfillSpec {
        collection_name: "tasks",
        root_fields: &[
            RootTimestampField {
                epoch_path: "next_attempt_at_epoch_seconds",
                datetime_path: "next_attempt_at",
            },
            RootTimestampField {
                epoch_path: "lease_expires_at_epoch_seconds",
                datetime_path: "lease_expires_at",
            },
            RootTimestampField {
                epoch_path: "last_heartbeat_at_epoch_seconds",
                datetime_path: "last_heartbeat_at",
            },
            RootTimestampField {
                epoch_path: "timed_out_at_epoch_seconds",
                datetime_path: "timed_out_at",
            },
            RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            },
            RootTimestampField {
                epoch_path: "updated_at_epoch_seconds",
                datetime_path: "updated_at",
            },
            RootTimestampField {
                epoch_path: "started_at_epoch_seconds",
                datetime_path: "started_at",
            },
            RootTimestampField {
                epoch_path: "finished_at_epoch_seconds",
                datetime_path: "finished_at",
            },
        ],
        array_fields: &[],
    },
];

pub async fn backfill_mongo_readable_dates(
    client: &mongodb::Client,
    database: &str,
) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let database = client.database(database);
    let mut total_modified_documents = 0;

    for spec in COLLECTION_SPECS {
        let collection = database.collection::<Document>(spec.collection_name);
        total_modified_documents += backfill_collection(&collection, spec).await?;
    }

    Ok(total_modified_documents)
}

async fn backfill_collection(
    collection: &Collection<Document>,
    spec: &CollectionBackfillSpec,
) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let mut cursor = collection
        .find(doc! {})
        .projection(build_projection(spec))
        .await?;
    let mut modified_documents = 0;

    while let Some(document) = cursor.try_next().await? {
        let Some(id) = document.get("_id").cloned() else {
            continue;
        };

        let Some(set_updates) = build_set_updates(&document, spec)? else {
            continue;
        };

        let update_result = collection
            .update_one(
                build_compare_and_set_filter(id, &document, spec),
                doc! { "$set": set_updates },
            )
            .await?;
        modified_documents += update_result.modified_count;
    }

    Ok(modified_documents)
}

fn build_projection(spec: &CollectionBackfillSpec) -> Document {
    let mut projection = doc! { "_id": 1 };

    for field in spec.root_fields {
        projection.insert(field.epoch_path, 1);
        projection.insert(field.datetime_path, 1);
    }

    for field in spec.array_fields {
        projection.insert(field.array_path, 1);
    }

    projection
}

fn build_set_updates(
    document: &Document,
    spec: &CollectionBackfillSpec,
) -> Result<Option<Document>, Box<dyn Error + Send + Sync>> {
    let mut set_updates = Document::new();

    for field in spec.root_fields {
        if let Some(datetime) = backfill_root_field(document, field)? {
            set_updates.insert(field.datetime_path, Bson::DateTime(datetime));
        }
    }

    for field in spec.array_fields {
        if let Some(array) = backfill_array_field(document, field)? {
            set_updates.insert(field.array_path, Bson::Array(array));
        }
    }

    if set_updates.is_empty() {
        Ok(None)
    } else {
        Ok(Some(set_updates))
    }
}

fn build_compare_and_set_filter(
    id: Bson,
    document: &Document,
    spec: &CollectionBackfillSpec,
) -> Document {
    let mut filter = doc! { "_id": id };

    for field in spec.root_fields {
        insert_filter_value_for_path(&mut filter, document, field.epoch_path);
        insert_filter_value_for_path(&mut filter, document, field.datetime_path);
    }

    for field in spec.array_fields {
        insert_filter_value_for_path(&mut filter, document, field.array_path);
    }

    filter
}

fn insert_filter_value_for_path(filter: &mut Document, document: &Document, path: &str) {
    match get_owned_bson_at_path(document, path) {
        Some(value) => {
            filter.insert(path, value);
        }
        None => {
            filter.insert(path, doc! { "$exists": false });
        }
    }
}

fn backfill_root_field(
    document: &Document,
    field: &RootTimestampField,
) -> Result<Option<DateTime>, Box<dyn Error + Send + Sync>> {
    if !path_is_missing_or_null(document, field.datetime_path) {
        return Ok(None);
    }

    let Some(epoch_seconds) = get_i64_at_path(document, field.epoch_path) else {
        return Ok(None);
    };

    Ok(Some(
        epoch_seconds_to_bson_datetime_with_field(epoch_seconds, field.datetime_path)
            .map_err(boxed_backfill_error)?,
    ))
}

fn backfill_array_field(
    document: &Document,
    field: &ArrayTimestampField,
) -> Result<Option<Vec<Bson>>, Box<dyn Error + Send + Sync>> {
    let Some(array) = get_array_at_path(document, field.array_path) else {
        return Ok(None);
    };

    let mut changed = false;
    let mut updated_array = Vec::with_capacity(array.len());

    for value in array {
        let Bson::Document(mut item) = value else {
            updated_array.push(value);
            continue;
        };

        if path_is_missing_or_null(&item, field.datetime_field) {
            if let Some(epoch_seconds) = get_i64_at_path(&item, field.epoch_field) {
                let datetime =
                    epoch_seconds_to_bson_datetime_with_field(epoch_seconds, field.datetime_field)
                        .map_err(boxed_backfill_error)?;
                set_bson_at_path(&mut item, field.datetime_field, Bson::DateTime(datetime));
                changed = true;
            }
        }

        updated_array.push(Bson::Document(item));
    }

    if changed {
        Ok(Some(updated_array))
    } else {
        Ok(None)
    }
}

fn get_i64_at_path(document: &Document, path: &str) -> Option<i64> {
    match get_bson_at_path(document, path) {
        Some(Bson::Int64(value)) => Some(*value),
        Some(Bson::Int32(value)) => Some(i64::from(*value)),
        _ => None,
    }
}

fn get_array_at_path(document: &Document, path: &str) -> Option<Vec<Bson>> {
    match get_bson_at_path(document, path) {
        Some(Bson::Array(array)) => Some(array.clone()),
        _ => None,
    }
}

fn path_is_missing_or_null(document: &Document, path: &str) -> bool {
    matches!(get_bson_at_path(document, path), None | Some(Bson::Null))
}

fn get_owned_bson_at_path(document: &Document, path: &str) -> Option<Bson> {
    get_bson_at_path(document, path).cloned()
}

fn get_bson_at_path<'a>(document: &'a Document, path: &str) -> Option<&'a Bson> {
    let mut segments = path.split('.');
    let first = segments.next()?;
    let mut current = document.get(first)?;

    for segment in segments {
        let Bson::Document(nested) = current else {
            return None;
        };
        current = nested.get(segment)?;
    }

    Some(current)
}

fn set_bson_at_path(document: &mut Document, path: &str, value: Bson) {
    let segments = path.split('.').collect::<Vec<_>>();
    set_bson_at_segments(document, &segments, value);
}

fn set_bson_at_segments(document: &mut Document, segments: &[&str], value: Bson) {
    if let Some((last_segment, prefix)) = segments.split_last() {
        if prefix.is_empty() {
            document.insert(*last_segment, value);
            return;
        }

        let entry = document
            .entry(prefix[0].to_string())
            .or_insert_with(|| Bson::Document(Document::new()));
        if !matches!(entry, Bson::Document(_)) {
            *entry = Bson::Document(Document::new());
        }

        let Bson::Document(nested) = entry else {
            return;
        };

        set_bson_at_segments(nested, &segments[1..], value);
    }
}

fn boxed_backfill_error(message: String) -> Box<dyn Error + Send + Sync> {
    Box::new(ReadableDatesBackfillError(message))
}

#[cfg(test)]
mod tests {
    use mongodb::bson::{doc, Bson, DateTime};

    use super::{
        build_set_updates, ArrayTimestampField, CollectionBackfillSpec, RootTimestampField,
    };

    #[test]
    fn build_set_updates_backfills_root_and_nested_fields() {
        let spec = CollectionBackfillSpec {
            collection_name: "test",
            root_fields: &[
                RootTimestampField {
                    epoch_path: "created_at_epoch_seconds",
                    datetime_path: "created_at",
                },
                RootTimestampField {
                    epoch_path: "nested.updated_at_epoch_seconds",
                    datetime_path: "nested.updated_at",
                },
            ],
            array_fields: &[],
        };
        let document = doc! {
            "_id": "doc-1",
            "created_at_epoch_seconds": 1_700_000_000_i64,
            "nested": {
                "updated_at_epoch_seconds": 1_700_000_100_i64,
            }
        };

        let updates = build_set_updates(&document, &spec)
            .expect("backfill updates should build")
            .expect("document should require updates");

        assert_eq!(
            updates.get("created_at"),
            Some(&Bson::DateTime(DateTime::from_millis(1_700_000_000_000)))
        );
        assert_eq!(
            updates.get("nested.updated_at"),
            Some(&Bson::DateTime(DateTime::from_millis(1_700_000_100_000)))
        );
    }

    #[test]
    fn build_set_updates_backfills_array_elements() {
        let spec = CollectionBackfillSpec {
            collection_name: "test",
            root_fields: &[],
            array_fields: &[ArrayTimestampField {
                array_path: "messages",
                epoch_field: "created_at_epoch_seconds",
                datetime_field: "created_at",
            }],
        };
        let document = doc! {
            "_id": "doc-1",
            "messages": [
                {
                    "id": "m1",
                    "created_at_epoch_seconds": 1_700_000_000_i64,
                },
                {
                    "id": "m2",
                    "created_at_epoch_seconds": 1_700_000_010_i64,
                    "created_at": DateTime::from_millis(1_700_000_010_000),
                }
            ]
        };

        let updates = build_set_updates(&document, &spec)
            .expect("backfill updates should build")
            .expect("document should require updates");
        let messages = updates
            .get_array("messages")
            .expect("messages array should be present");
        let first = messages[0]
            .as_document()
            .expect("first message should stay a document");
        let second = messages[1]
            .as_document()
            .expect("second message should stay a document");

        assert_eq!(
            first.get("created_at"),
            Some(&Bson::DateTime(DateTime::from_millis(1_700_000_000_000)))
        );
        assert_eq!(
            second.get("created_at"),
            Some(&Bson::DateTime(DateTime::from_millis(1_700_000_010_000)))
        );
    }

    #[test]
    fn build_set_updates_is_idempotent_when_readable_dates_already_exist() {
        let spec = CollectionBackfillSpec {
            collection_name: "test",
            root_fields: &[RootTimestampField {
                epoch_path: "created_at_epoch_seconds",
                datetime_path: "created_at",
            }],
            array_fields: &[ArrayTimestampField {
                array_path: "attempts",
                epoch_field: "recorded_at_epoch_seconds",
                datetime_field: "recorded_at",
            }],
        };
        let document = doc! {
            "_id": "doc-1",
            "created_at_epoch_seconds": 1_700_000_000_i64,
            "created_at": DateTime::from_millis(1_700_000_000_000),
            "attempts": [
                {
                    "recorded_at_epoch_seconds": 1_700_000_010_i64,
                    "recorded_at": DateTime::from_millis(1_700_000_010_000),
                }
            ]
        };

        let updates = build_set_updates(&document, &spec).expect("backfill should not fail");

        assert_eq!(updates, None);
    }
}
