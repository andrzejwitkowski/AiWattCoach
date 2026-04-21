use super::*;

impl<Tasks, Workers, Time> TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    pub(super) fn build_claim_request(
        &self,
        worker_id: &str,
        enabled_task_types: Vec<String>,
        is_leader: bool,
        lease_duration_seconds: i64,
    ) -> Result<TaskClaimRequest, TaskSchedulerError> {
        let now_epoch_seconds = validate_positive_duration(
            lease_duration_seconds,
            "task lease duration must be positive",
        )
        .map(|_| self.clock.now_epoch_seconds())?;
        Ok(TaskClaimRequest {
            worker_id: worker_id.to_string(),
            enabled_task_types,
            is_leader,
            now_epoch_seconds,
            lease_expires_at_epoch_seconds: now_epoch_seconds + lease_duration_seconds,
        })
    }

    pub(super) fn build_heartbeat_request(
        &self,
        task_id: &str,
        worker_id: &str,
        lease_duration_seconds: i64,
    ) -> Result<TaskHeartbeatRequest, TaskSchedulerError> {
        let now_epoch_seconds = validate_positive_duration(
            lease_duration_seconds,
            "task lease duration must be positive",
        )
        .map(|_| self.clock.now_epoch_seconds())?;
        Ok(TaskHeartbeatRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            last_heartbeat_at_epoch_seconds: now_epoch_seconds,
            lease_expires_at_epoch_seconds: now_epoch_seconds + lease_duration_seconds,
        })
    }

    pub(super) fn build_checkpoint_request(
        &self,
        task_id: &str,
        worker_id: &str,
        checkpoint: serde_json::Value,
    ) -> TaskCheckpointRequest {
        TaskCheckpointRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            checkpoint,
            updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }

    pub(super) fn build_complete_request(
        &self,
        task_id: &str,
        worker_id: &str,
        checkpoint: Option<serde_json::Value>,
    ) -> TaskCompleteRequest {
        TaskCompleteRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            checkpoint,
            completed_at_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }

    pub(super) fn build_fail_request(&self, input: FailTaskInput<'_>) -> TaskFailRequest {
        let failed_at_epoch_seconds = self.clock.now_epoch_seconds();
        let retry_at_epoch_seconds = input.retryable.then(|| {
            input
                .retry_strategy
                .next_retry_at(input.attempt_count, failed_at_epoch_seconds)
        });
        TaskFailRequest {
            task_id: input.task_id.to_string(),
            worker_id: input.worker_id.to_string(),
            checkpoint: input.checkpoint,
            error_message: input.error_message,
            failed_at_epoch_seconds,
            retry_at_epoch_seconds: retry_at_epoch_seconds.flatten(),
        }
    }
}
