use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::error::is_duplicate_key_error;
use crate::domain::task_scheduler::{
    RetryStrategy, ScheduledTask, TaskClaimRequest, TaskEnqueueResult, TaskHeartbeatRequest,
    TaskListFilter, TaskMarkTimedOutRequest, TaskRecoverRequest, TaskRepository, TaskRetryRequest,
    TaskSchedulerError, TaskStatus,
};

#[derive(Clone)]
pub struct MongoTaskRepository {
    collection: Collection<TaskDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TaskDocument {
    #[serde(rename = "_id")]
    id: String,
    user_id: String,
    task_type: String,
    status: String,
    payload: serde_json::Value,
    checkpoint: Option<serde_json::Value>,
    retry_strategy: RetryStrategyDocument,
    dedupe_key: String,
    error_message: Option<String>,
    attempt_count: i64,
    next_attempt_at_epoch_seconds: i64,
    claimed_by: Option<String>,
    lease_expires_at_epoch_seconds: Option<i64>,
    last_heartbeat_at_epoch_seconds: Option<i64>,
    execution_timeout_seconds: i64,
    timed_out_at_epoch_seconds: Option<i64>,
    leader_only: bool,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
    started_at_epoch_seconds: Option<i64>,
    finished_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RetryStrategyDocument {
    kind: String,
    max_attempts: Option<i64>,
    delay_seconds: Option<i64>,
    initial_delay_seconds: Option<i64>,
    max_delay_seconds: Option<i64>,
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

impl TaskRepository for MongoTaskRepository {
    fn enqueue_if_absent(
        &self,
        task: ScheduledTask,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_task_to_document(&task)?;
            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(storage_error(error))
                    }
                })?;

            if inserted {
                return Ok(TaskEnqueueResult {
                    task,
                    created: true,
                });
            }

            let existing =
                Self::reload_existing_after_duplicate_insert(&collection, &document).await?;

            Ok(TaskEnqueueResult {
                task: map_document_to_task(existing)?,
                created: false,
            })
        })
    }

    fn claim_next_due(
        &self,
        request: TaskClaimRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        Box::pin(async move {
            if request.enabled_task_types.is_empty() {
                return Ok(None);
            }

            let leader_filter = if request.is_leader {
                doc! {}
            } else {
                doc! { "leader_only": false }
            };

            let mut filter = doc! {
                "task_type": { "$in": request.enabled_task_types },
                "status": { "$in": [status_as_str(&TaskStatus::Queued), status_as_str(&TaskStatus::RetryScheduled)] },
                "next_attempt_at_epoch_seconds": { "$lte": request.now_epoch_seconds },
            };
            filter.extend(leader_filter);

            let updated = collection
                .find_one_and_update(
                    filter,
                    doc! {
                        "$set": {
                            "status": status_as_str(&TaskStatus::Running),
                            "claimed_by": &request.worker_id,
                            "lease_expires_at_epoch_seconds": request.lease_expires_at_epoch_seconds,
                            "last_heartbeat_at_epoch_seconds": request.now_epoch_seconds,
                            "updated_at_epoch_seconds": request.now_epoch_seconds,
                            "started_at_epoch_seconds": request.now_epoch_seconds,
                            "finished_at_epoch_seconds": Bson::Null,
                            "timed_out_at_epoch_seconds": Bson::Null,
                        },
                        "$inc": { "attempt_count": 1_i64 },
                    },
                )
                .sort(doc! { "next_attempt_at_epoch_seconds": 1, "created_at_epoch_seconds": 1 })
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .map_err(storage_error)?;

            updated.map(map_document_to_task).transpose()
        })
    }

    fn heartbeat(
        &self,
        request: TaskHeartbeatRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        Box::pin(async move {
            let updated = collection
                .find_one_and_update(
                    doc! {
                        "_id": &request.task_id,
                        "status": status_as_str(&TaskStatus::Running),
                        "claimed_by": &request.worker_id,
                    },
                    doc! {
                        "$set": {
                            "last_heartbeat_at_epoch_seconds": request.last_heartbeat_at_epoch_seconds,
                            "lease_expires_at_epoch_seconds": request.lease_expires_at_epoch_seconds,
                            "updated_at_epoch_seconds": request.last_heartbeat_at_epoch_seconds,
                        },
                    },
                )
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .map_err(storage_error)?;

            updated.map(map_document_to_task).transpose()
        })
    }

    fn list_timeout_candidates(
        &self,
        now_epoch_seconds: i64,
        limit: usize,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        Box::pin(async move {
            if limit == 0 {
                return Ok(Vec::new());
            }

            let documents = collection
                .find(Self::timeout_candidate_filter(now_epoch_seconds))
                .sort(doc! { "lease_expires_at_epoch_seconds": 1, "updated_at_epoch_seconds": 1 })
                .limit(limit as i64)
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)?;

            documents
                .into_iter()
                .map(map_document_to_task)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn mark_timed_out(
        &self,
        request: TaskMarkTimedOutRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<bool, TaskSchedulerError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let mut filter = doc! {
                "_id": &request.task_id,
                "status": status_as_str(&TaskStatus::Running),
                "updated_at_epoch_seconds": request.expected_updated_at_epoch_seconds,
            };

            match request.expected_claimed_by.as_deref() {
                Some(worker_id) => {
                    filter.insert("claimed_by", worker_id);
                }
                None => {
                    filter.insert(
                        "$or",
                        vec![
                            Bson::Document(doc! { "claimed_by": { "$exists": false } }),
                            Bson::Document(doc! { "claimed_by": Bson::Null }),
                        ],
                    );
                }
            }

            let result = collection
                .update_one(
                    filter,
                    doc! {
                        "$set": {
                            "status": status_as_str(&TaskStatus::TimedOut),
                            "timed_out_at_epoch_seconds": request.timed_out_at_epoch_seconds,
                            "updated_at_epoch_seconds": request.timed_out_at_epoch_seconds,
                            "finished_at_epoch_seconds": request.timed_out_at_epoch_seconds,
                        },
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "last_heartbeat_at_epoch_seconds": "",
                        },
                    },
                )
                .await
                .map_err(storage_error)?;

            Ok(result.modified_count > 0)
        })
    }

    fn recover(
        &self,
        request: TaskRecoverRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<bool, TaskSchedulerError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let mut filter = doc! {
                "_id": &request.task_id,
                "status": status_as_str(&TaskStatus::Running),
                "updated_at_epoch_seconds": request.expected_updated_at_epoch_seconds,
            };

            match request.expected_claimed_by.as_deref() {
                Some(worker_id) => {
                    filter.insert("claimed_by", worker_id);
                }
                None => {
                    filter.insert(
                        "$or",
                        vec![
                            Bson::Document(doc! { "claimed_by": { "$exists": false } }),
                            Bson::Document(doc! { "claimed_by": Bson::Null }),
                        ],
                    );
                }
            }

            let result = collection
                .update_one(
                    filter,
                    doc! {
                        "$set": {
                            "status": status_as_str(&TaskStatus::RetryScheduled),
                            "next_attempt_at_epoch_seconds": request.recovered_at_epoch_seconds,
                            "updated_at_epoch_seconds": request.recovered_at_epoch_seconds,
                            "started_at_epoch_seconds": Bson::Null,
                            "finished_at_epoch_seconds": Bson::Null,
                            "timed_out_at_epoch_seconds": Bson::Null,
                        },
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "last_heartbeat_at_epoch_seconds": "",
                        },
                    },
                )
                .await
                .map_err(storage_error)?;

            Ok(result.modified_count > 0)
        })
    }

    fn retry(
        &self,
        request: TaskRetryRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        Box::pin(async move {
            let updated = collection
                .find_one_and_update(
                    doc! {
                        "_id": &request.task_id,
                        "status": {
                            "$in": [
                                status_as_str(&TaskStatus::Failed),
                                status_as_str(&TaskStatus::TimedOut),
                            ],
                        },
                    },
                    doc! {
                        "$set": {
                            "status": status_as_str(&TaskStatus::Queued),
                            "next_attempt_at_epoch_seconds": request.retried_at_epoch_seconds,
                            "updated_at_epoch_seconds": request.retried_at_epoch_seconds,
                            "timed_out_at_epoch_seconds": Bson::Null,
                            "started_at_epoch_seconds": Bson::Null,
                            "finished_at_epoch_seconds": Bson::Null,
                        },
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "error_message": "",
                        },
                    },
                )
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .map_err(storage_error)?;

            updated.map(map_document_to_task).transpose()
        })
    }

    fn find_by_id(
        &self,
        task_id: &str,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "_id": &task_id })
                .await
                .map_err(storage_error)?;
            document.map(map_document_to_task).transpose()
        })
    }

    fn list(
        &self,
        filter: TaskListFilter,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>>
    {
        let collection = self.collection.clone();
        Box::pin(async move {
            let mut mongo_filter = doc! {};
            if !filter.task_types.is_empty() {
                mongo_filter.insert("task_type", doc! { "$in": filter.task_types });
            }
            if !filter.statuses.is_empty() {
                mongo_filter.insert(
                    "status",
                    doc! {
                        "$in": filter
                            .statuses
                            .iter()
                            .map(status_as_str)
                            .collect::<Vec<_>>()
                    },
                );
            }
            if let Some(user_id) = filter.user_id {
                mongo_filter.insert("user_id", user_id);
            }

            let documents = collection
                .find(mongo_filter)
                .sort(doc! { "updated_at_epoch_seconds": -1, "created_at_epoch_seconds": -1 })
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)?;

            documents
                .into_iter()
                .map(map_document_to_task)
                .collect::<Result<Vec<_>, _>>()
        })
    }
}

fn storage_error(error: mongodb::error::Error) -> TaskSchedulerError {
    TaskSchedulerError::Repository(error.to_string())
}

fn status_as_str(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::RetryScheduled => "retry_scheduled",
        TaskStatus::Failed => "failed",
        TaskStatus::Completed => "completed",
        TaskStatus::TimedOut => "timed_out",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn map_status(value: &str) -> Result<TaskStatus, TaskSchedulerError> {
    match value {
        "queued" => Ok(TaskStatus::Queued),
        "running" => Ok(TaskStatus::Running),
        "retry_scheduled" => Ok(TaskStatus::RetryScheduled),
        "failed" => Ok(TaskStatus::Failed),
        "completed" => Ok(TaskStatus::Completed),
        "timed_out" => Ok(TaskStatus::TimedOut),
        "cancelled" => Ok(TaskStatus::Cancelled),
        other => Err(TaskSchedulerError::Repository(format!(
            "unknown task status: {other}",
        ))),
    }
}

fn map_retry_strategy(strategy: &RetryStrategy) -> RetryStrategyDocument {
    match strategy {
        RetryStrategy::Never => RetryStrategyDocument {
            kind: "never".to_string(),
            max_attempts: Some(1),
            delay_seconds: None,
            initial_delay_seconds: None,
            max_delay_seconds: None,
        },
        RetryStrategy::Fixed {
            max_attempts,
            delay_seconds,
        } => RetryStrategyDocument {
            kind: "fixed".to_string(),
            max_attempts: Some(i64::from(*max_attempts)),
            delay_seconds: Some(*delay_seconds),
            initial_delay_seconds: None,
            max_delay_seconds: None,
        },
        RetryStrategy::Exponential {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        } => RetryStrategyDocument {
            kind: "exponential".to_string(),
            max_attempts: Some(i64::from(*max_attempts)),
            delay_seconds: None,
            initial_delay_seconds: Some(*initial_delay_seconds),
            max_delay_seconds: Some(*max_delay_seconds),
        },
    }
}

fn map_retry_strategy_document(
    document: RetryStrategyDocument,
) -> Result<RetryStrategy, TaskSchedulerError> {
    match document.kind.as_str() {
        "never" => Ok(RetryStrategy::Never),
        "fixed" => Ok(RetryStrategy::Fixed {
            max_attempts: parse_u32_field(document.max_attempts, "fixed retry max_attempts")?,
            delay_seconds: document.delay_seconds.ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "fixed retry strategy missing delay_seconds".to_string(),
                )
            })?,
        }),
        "exponential" => Ok(RetryStrategy::Exponential {
            max_attempts: parse_u32_field(document.max_attempts, "exponential retry max_attempts")?,
            initial_delay_seconds: document.initial_delay_seconds.ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "exponential retry strategy missing initial_delay_seconds".to_string(),
                )
            })?,
            max_delay_seconds: document.max_delay_seconds.ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "exponential retry strategy missing max_delay_seconds".to_string(),
                )
            })?,
        }),
        other => Err(TaskSchedulerError::Repository(format!(
            "unknown retry strategy kind: {other}",
        ))),
    }
}

fn parse_u32_field(value: Option<i64>, field_name: &str) -> Result<u32, TaskSchedulerError> {
    let value = value.ok_or_else(|| {
        TaskSchedulerError::Repository(format!("missing {field_name} in task retry strategy"))
    })?;
    u32::try_from(value)
        .map_err(|_| TaskSchedulerError::Repository(format!("invalid {field_name}: {value}")))
}

fn map_task_to_document(task: &ScheduledTask) -> Result<TaskDocument, TaskSchedulerError> {
    Ok(TaskDocument {
        id: task.id.clone(),
        user_id: task.user_id.clone(),
        task_type: task.task_type.clone(),
        status: status_as_str(&task.status).to_string(),
        payload: task.payload.clone(),
        checkpoint: task.checkpoint.clone(),
        retry_strategy: map_retry_strategy(&task.retry_strategy),
        dedupe_key: task.dedupe_key.clone(),
        error_message: task.error_message.clone(),
        attempt_count: i64::from(task.attempt_count),
        next_attempt_at_epoch_seconds: task.next_attempt_at_epoch_seconds,
        claimed_by: task.claimed_by.clone(),
        lease_expires_at_epoch_seconds: task.lease_expires_at_epoch_seconds,
        last_heartbeat_at_epoch_seconds: task.last_heartbeat_at_epoch_seconds,
        execution_timeout_seconds: task.execution_timeout_seconds,
        timed_out_at_epoch_seconds: task.timed_out_at_epoch_seconds,
        leader_only: task.leader_only,
        created_at_epoch_seconds: task.created_at_epoch_seconds,
        updated_at_epoch_seconds: task.updated_at_epoch_seconds,
        started_at_epoch_seconds: task.started_at_epoch_seconds,
        finished_at_epoch_seconds: task.finished_at_epoch_seconds,
    })
}

fn map_document_to_task(document: TaskDocument) -> Result<ScheduledTask, TaskSchedulerError> {
    Ok(ScheduledTask {
        id: document.id,
        user_id: document.user_id,
        task_type: document.task_type,
        status: map_status(&document.status)?,
        payload: document.payload,
        checkpoint: document.checkpoint,
        retry_strategy: map_retry_strategy_document(document.retry_strategy)?,
        dedupe_key: document.dedupe_key,
        error_message: document.error_message,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            TaskSchedulerError::Repository("invalid task attempt_count".to_string())
        })?,
        next_attempt_at_epoch_seconds: document.next_attempt_at_epoch_seconds,
        claimed_by: document.claimed_by,
        lease_expires_at_epoch_seconds: document.lease_expires_at_epoch_seconds,
        last_heartbeat_at_epoch_seconds: document.last_heartbeat_at_epoch_seconds,
        execution_timeout_seconds: document.execution_timeout_seconds,
        timed_out_at_epoch_seconds: document.timed_out_at_epoch_seconds,
        leader_only: document.leader_only,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
        started_at_epoch_seconds: document.started_at_epoch_seconds,
        finished_at_epoch_seconds: document.finished_at_epoch_seconds,
    })
}
