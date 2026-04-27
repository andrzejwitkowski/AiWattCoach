use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{optional_epoch_seconds_to_bson_datetime, resolve_required_epoch_seconds};
use crate::domain::task_scheduler::{TaskSchedulerError, TaskWorker, TaskWorkerRepository};

#[derive(Clone)]
pub struct MongoTaskWorkerRepository {
    collection: Collection<TaskWorkerDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TaskWorkerDocument {
    #[serde(rename = "_id")]
    worker_id: String,
    is_leader: bool,
    enabled_task_types: Vec<String>,
    active_task_ids: Vec<String>,
    last_heartbeat_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_heartbeat_at: Option<DateTime>,
}

impl MongoTaskWorkerRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("task_workers"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TaskSchedulerError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "last_heartbeat_at_epoch_seconds": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("task_workers_last_heartbeat_lookup".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "enabled_task_types": 1, "is_leader": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("task_workers_enabled_types_lookup".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| TaskSchedulerError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl TaskWorkerRepository for MongoTaskWorkerRepository {
    fn upsert(
        &self,
        worker: TaskWorker,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = TaskWorkerDocument {
                worker_id: worker.worker_id.clone(),
                is_leader: worker.is_leader,
                enabled_task_types: worker.enabled_task_types.clone(),
                active_task_ids: worker.active_task_ids.clone(),
                last_heartbeat_at_epoch_seconds: Some(worker.last_heartbeat_at_epoch_seconds),
                last_heartbeat_at: optional_epoch_seconds_to_bson_datetime(
                    Some(worker.last_heartbeat_at_epoch_seconds),
                    "last_heartbeat_at",
                )
                .expect("last_heartbeat_at should fit BSON DateTime"),
            };
            collection
                .replace_one(doc! { "_id": &document.worker_id }, &document)
                .upsert(true)
                .await
                .map_err(|error| TaskSchedulerError::Repository(error.to_string()))?;
            Ok(worker)
        })
    }

    fn touch_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        last_heartbeat_at_epoch_seconds: i64,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let collection = self.collection.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            let updated = collection
                .find_one_and_update(
                    doc! { "_id": &worker_id },
                    doc! {
                        "$set": {
                            "is_leader": is_leader,
                            "enabled_task_types": &enabled_task_types,
                            "active_task_ids": Vec::<String>::new(),
                            "last_heartbeat_at_epoch_seconds": last_heartbeat_at_epoch_seconds,
                            "last_heartbeat_at": optional_epoch_seconds_to_bson_datetime(
                                Some(last_heartbeat_at_epoch_seconds),
                                "last_heartbeat_at",
                            )
                            .expect("last_heartbeat_at should fit BSON DateTime"),
                        },
                    },
                )
                .upsert(true)
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .map_err(|error| TaskSchedulerError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    TaskSchedulerError::Repository(
                        "worker heartbeat update disappeared before reload".to_string(),
                    )
                })?;

            Ok(TaskWorker {
                worker_id: updated.worker_id,
                is_leader: updated.is_leader,
                enabled_task_types: updated.enabled_task_types,
                active_task_ids: updated.active_task_ids,
                last_heartbeat_at_epoch_seconds: resolve_required_epoch_seconds(
                    updated.last_heartbeat_at,
                    updated.last_heartbeat_at_epoch_seconds,
                    "last_heartbeat_at",
                )
                .expect("task worker documents must store last_heartbeat_at"),
            })
        })
    }

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "_id": &worker_id })
                .await
                .map_err(|error| TaskSchedulerError::Repository(error.to_string()))?;
            Ok(document.map(|document| TaskWorker {
                worker_id: document.worker_id,
                is_leader: document.is_leader,
                enabled_task_types: document.enabled_task_types,
                active_task_ids: document.active_task_ids,
                last_heartbeat_at_epoch_seconds: resolve_required_epoch_seconds(
                    document.last_heartbeat_at,
                    document.last_heartbeat_at_epoch_seconds,
                    "last_heartbeat_at",
                )
                .expect("task worker documents must store last_heartbeat_at"),
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::TaskWorkerDocument;
    use crate::domain::task_scheduler::TaskWorker;

    #[test]
    fn task_worker_document_reads_datetime_field_without_legacy_epoch() {
        let document = TaskWorkerDocument {
            worker_id: "worker-1".to_string(),
            is_leader: true,
            enabled_task_types: vec!["summary".to_string()],
            active_task_ids: vec!["task-1".to_string()],
            last_heartbeat_at_epoch_seconds: None,
            last_heartbeat_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_000_000)),
        };

        let worker = TaskWorker {
            worker_id: document.worker_id,
            is_leader: document.is_leader,
            enabled_task_types: document.enabled_task_types,
            active_task_ids: document.active_task_ids,
            last_heartbeat_at_epoch_seconds: super::resolve_required_epoch_seconds(
                document.last_heartbeat_at,
                document.last_heartbeat_at_epoch_seconds,
                "last_heartbeat_at",
            )
            .expect("datetime-backed task worker should map"),
        };

        assert_eq!(worker.last_heartbeat_at_epoch_seconds, 1_700_000_000);
    }
}
