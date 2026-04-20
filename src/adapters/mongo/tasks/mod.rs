use mongodb::{
    bson::{doc, Bson},
    options::IndexOptions,
    Collection, IndexModel,
};

use crate::domain::task_scheduler::{TaskSchedulerError, TaskStatus};

mod document;
mod mapping;
mod repository;

use document::TaskDocument;
use mapping::{status_as_str, storage_error};

#[derive(Clone)]
pub struct MongoTaskRepository {
    collection: Collection<TaskDocument>,
}

impl MongoTaskRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client.database(database.as_ref()).collection("tasks"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TaskSchedulerError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "dedupe_key": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("tasks_dedupe_key_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! {
                        "task_type": 1,
                        "status": 1,
                        "leader_only": 1,
                        "next_attempt_at_epoch_seconds": 1,
                        "created_at_epoch_seconds": 1,
                    })
                    .options(
                        IndexOptions::builder()
                            .name("tasks_claim_due_lookup".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "status": 1, "lease_expires_at_epoch_seconds": 1, "updated_at_epoch_seconds": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("tasks_timeout_lookup".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "claimed_by": 1, "status": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("tasks_claimed_by_status_lookup".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    fn timeout_candidate_filter(now_epoch_seconds: i64) -> mongodb::bson::Document {
        doc! {
            "status": status_as_str(&TaskStatus::Running),
            "$or": [
                {
                    "lease_expires_at_epoch_seconds": {
                        "$lte": now_epoch_seconds,
                    },
                },
                {
                    "$expr": {
                        "$and": [
                            { "$ne": ["$started_at_epoch_seconds", Bson::Null] },
                            {
                                "$lte": [
                                    {
                                        "$add": [
                                            "$started_at_epoch_seconds",
                                            "$execution_timeout_seconds",
                                        ],
                                    },
                                    now_epoch_seconds,
                                ],
                            },
                        ],
                    },
                },
            ],
        }
    }

    async fn reload_existing_after_duplicate_insert(
        collection: &Collection<TaskDocument>,
        document: &TaskDocument,
    ) -> Result<TaskDocument, TaskSchedulerError> {
        if let Some(existing) = collection
            .find_one(doc! { "dedupe_key": &document.dedupe_key, "user_id": &document.user_id })
            .await
            .map_err(storage_error)?
        {
            return Ok(existing);
        }

        collection
            .find_one(doc! { "_id": &document.id })
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "task with duplicate key disappeared before reload".to_string(),
                )
            })
    }
}
