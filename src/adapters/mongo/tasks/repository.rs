use futures::TryStreamExt;
use mongodb::bson::{doc, Bson, DateTime};

use super::super::error::is_duplicate_key_error;
use super::mapping::{
    bson_datetime, map_document_to_task, map_task_to_document, status_as_str, storage_error,
    terminal_task_cleanup_bson,
};
use super::MongoTaskRepository;
use crate::domain::task_scheduler::{
    TaskCheckpointRequest, TaskClaimRequest, TaskCompleteRequest, TaskEnqueueResult,
    TaskFailRequest, TaskHeartbeatRequest, TaskListFilter, TaskMarkTimedOutRequest,
    TaskRecoverRequest, TaskRepository, TaskRetryRequest, TaskSchedulerError, TaskStatus,
};

impl TaskRepository for MongoTaskRepository {
    fn enqueue_if_absent(
        &self,
        task: crate::domain::task_scheduler::ScheduledTask,
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
                MongoTaskRepository::reload_existing_after_duplicate_insert(&collection, &document)
                    .await?;

            Ok(TaskEnqueueResult {
                task: map_document_to_task(existing)?,
                created: false,
            })
        })
    }

    fn claim_next_due(
        &self,
        request: TaskClaimRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
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
                            "lease_expires_at": bson_or_null(
                                bson_datetime(Some(request.lease_expires_at_epoch_seconds), "lease_expires_at")?
                            ),
                            "last_heartbeat_at_epoch_seconds": request.now_epoch_seconds,
                            "last_heartbeat_at": bson_or_null(
                                bson_datetime(Some(request.now_epoch_seconds), "last_heartbeat_at")?
                            ),
                            "updated_at_epoch_seconds": request.now_epoch_seconds,
                            "updated_at": bson_or_null(
                                bson_datetime(Some(request.now_epoch_seconds), "updated_at")?
                            ),
                            "started_at_epoch_seconds": request.now_epoch_seconds,
                            "started_at": bson_or_null(
                                bson_datetime(Some(request.now_epoch_seconds), "started_at")?
                            ),
                            "finished_at_epoch_seconds": Bson::Null,
                            "finished_at": Bson::Null,
                            "timed_out_at_epoch_seconds": Bson::Null,
                            "timed_out_at": Bson::Null,
                            "cleanup_after": Bson::Null,
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
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
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
                            "last_heartbeat_at": bson_or_null(
                                bson_datetime(Some(request.last_heartbeat_at_epoch_seconds), "last_heartbeat_at")?
                            ),
                            "lease_expires_at_epoch_seconds": request.lease_expires_at_epoch_seconds,
                            "lease_expires_at": bson_or_null(
                                bson_datetime(Some(request.lease_expires_at_epoch_seconds), "lease_expires_at")?
                            ),
                            "updated_at_epoch_seconds": request.last_heartbeat_at_epoch_seconds,
                            "updated_at": bson_or_null(
                                bson_datetime(Some(request.last_heartbeat_at_epoch_seconds), "updated_at")?
                            ),
                        },
                    },
                )
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .map_err(storage_error)?;

            updated.map(map_document_to_task).transpose()
        })
    }

    fn save_checkpoint(
        &self,
        request: TaskCheckpointRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
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
                            "checkpoint": mongodb::bson::to_bson(&request.checkpoint).map_err(|error| TaskSchedulerError::Repository(error.to_string()))?,
                            "updated_at_epoch_seconds": request.updated_at_epoch_seconds,
                            "updated_at": bson_or_null(
                                bson_datetime(Some(request.updated_at_epoch_seconds), "updated_at")?
                            ),
                        },
                    },
                )
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .map_err(storage_error)?;

            updated.map(map_document_to_task).transpose()
        })
    }

    fn complete(
        &self,
        request: TaskCompleteRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
        let collection = self.collection.clone();
        Box::pin(async move {
            let mut set = doc! {
                "status": status_as_str(&TaskStatus::Completed),
                "updated_at_epoch_seconds": request.completed_at_epoch_seconds,
                "updated_at": bson_or_null(
                    bson_datetime(Some(request.completed_at_epoch_seconds), "updated_at")?
                ),
                "finished_at_epoch_seconds": request.completed_at_epoch_seconds,
                "finished_at": bson_or_null(
                    bson_datetime(Some(request.completed_at_epoch_seconds), "finished_at")?
                ),
                "timed_out_at_epoch_seconds": Bson::Null,
                "timed_out_at": Bson::Null,
                "cleanup_after": terminal_task_cleanup_bson(&TaskStatus::Completed, request.completed_at_epoch_seconds)?,
            };
            if let Some(checkpoint) = request.checkpoint {
                set.insert(
                    "checkpoint",
                    mongodb::bson::to_bson(&checkpoint)
                        .map_err(|error| TaskSchedulerError::Repository(error.to_string()))?,
                );
            }

            let updated = collection
                .find_one_and_update(
                    doc! {
                        "_id": &request.task_id,
                        "status": status_as_str(&TaskStatus::Running),
                        "claimed_by": &request.worker_id,
                    },
                    doc! {
                        "$set": set,
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "lease_expires_at": "",
                            "last_heartbeat_at_epoch_seconds": "",
                            "last_heartbeat_at": "",
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

    fn fail(
        &self,
        request: TaskFailRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
        let collection = self.collection.clone();
        Box::pin(async move {
            let next_status = if request.retry_at_epoch_seconds.is_some() {
                TaskStatus::RetryScheduled
            } else {
                TaskStatus::Failed
            };

            let mut set = doc! {
                "status": status_as_str(&next_status),
                "error_message": request.error_message,
                "updated_at_epoch_seconds": request.failed_at_epoch_seconds,
                "updated_at": bson_or_null(
                    bson_datetime(Some(request.failed_at_epoch_seconds), "updated_at")?
                ),
                "next_attempt_at_epoch_seconds": request.retry_at_epoch_seconds.unwrap_or(request.failed_at_epoch_seconds),
                "next_attempt_at": bson_or_null(
                    bson_datetime(
                        Some(request.retry_at_epoch_seconds.unwrap_or(request.failed_at_epoch_seconds)),
                        "next_attempt_at",
                    )?
                ),
                "finished_at_epoch_seconds": if matches!(next_status, TaskStatus::Failed) { Bson::Int64(request.failed_at_epoch_seconds) } else { Bson::Null },
                "finished_at": if matches!(next_status, TaskStatus::Failed) {
                    bson_or_null(bson_datetime(Some(request.failed_at_epoch_seconds), "finished_at")?)
                } else {
                    Bson::Null
                },
                "cleanup_after": if matches!(next_status, TaskStatus::Failed) { terminal_task_cleanup_bson(&TaskStatus::Failed, request.failed_at_epoch_seconds)? } else { Bson::Null },
            };
            if let Some(checkpoint) = request.checkpoint {
                set.insert(
                    "checkpoint",
                    mongodb::bson::to_bson(&checkpoint)
                        .map_err(|error| TaskSchedulerError::Repository(error.to_string()))?,
                );
            }

            let updated = collection
                .find_one_and_update(
                    doc! {
                        "_id": &request.task_id,
                        "status": status_as_str(&TaskStatus::Running),
                        "claimed_by": &request.worker_id,
                    },
                    doc! {
                        "$set": set,
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "lease_expires_at": "",
                            "last_heartbeat_at_epoch_seconds": "",
                            "last_heartbeat_at": "",
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
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Vec<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
        let collection = self.collection.clone();
        Box::pin(async move {
            if limit == 0 {
                return Ok(Vec::new());
            }

            let documents = collection
                .find(MongoTaskRepository::timeout_candidate_filter(
                    now_epoch_seconds,
                ))
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
                            "timed_out_at": bson_or_null(
                                bson_datetime(Some(request.timed_out_at_epoch_seconds), "timed_out_at")?
                            ),
                            "updated_at_epoch_seconds": request.timed_out_at_epoch_seconds,
                            "updated_at": bson_or_null(
                                bson_datetime(Some(request.timed_out_at_epoch_seconds), "updated_at")?
                            ),
                            "finished_at_epoch_seconds": request.timed_out_at_epoch_seconds,
                            "finished_at": bson_or_null(
                                bson_datetime(Some(request.timed_out_at_epoch_seconds), "finished_at")?
                            ),
                            "cleanup_after": terminal_task_cleanup_bson(&TaskStatus::TimedOut, request.timed_out_at_epoch_seconds)?,
                        },
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "lease_expires_at": "",
                            "last_heartbeat_at_epoch_seconds": "",
                            "last_heartbeat_at": "",
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
                            "next_attempt_at": bson_or_null(
                                bson_datetime(Some(request.recovered_at_epoch_seconds), "next_attempt_at")?
                            ),
                            "updated_at_epoch_seconds": request.recovered_at_epoch_seconds,
                            "updated_at": bson_or_null(
                                bson_datetime(Some(request.recovered_at_epoch_seconds), "updated_at")?
                            ),
                            "started_at_epoch_seconds": Bson::Null,
                            "started_at": Bson::Null,
                            "finished_at_epoch_seconds": Bson::Null,
                            "finished_at": Bson::Null,
                            "timed_out_at_epoch_seconds": Bson::Null,
                            "timed_out_at": Bson::Null,
                            "cleanup_after": Bson::Null,
                        },
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "lease_expires_at": "",
                            "last_heartbeat_at_epoch_seconds": "",
                            "last_heartbeat_at": "",
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
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
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
                            "next_attempt_at": bson_or_null(
                                bson_datetime(Some(request.retried_at_epoch_seconds), "next_attempt_at")?
                            ),
                            "updated_at_epoch_seconds": request.retried_at_epoch_seconds,
                            "updated_at": bson_or_null(
                                bson_datetime(Some(request.retried_at_epoch_seconds), "updated_at")?
                            ),
                            "timed_out_at_epoch_seconds": Bson::Null,
                            "timed_out_at": Bson::Null,
                            "started_at_epoch_seconds": Bson::Null,
                            "started_at": Bson::Null,
                            "finished_at_epoch_seconds": Bson::Null,
                            "finished_at": Bson::Null,
                            "cleanup_after": Bson::Null,
                        },
                        "$unset": {
                            "claimed_by": "",
                            "lease_expires_at_epoch_seconds": "",
                            "lease_expires_at": "",
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
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Option<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
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
    ) -> crate::domain::task_scheduler::BoxFuture<
        Result<Vec<crate::domain::task_scheduler::ScheduledTask>, TaskSchedulerError>,
    > {
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

fn bson_or_null(value: Option<DateTime>) -> Bson {
    value.map(Bson::DateTime).unwrap_or(Bson::Null)
}
