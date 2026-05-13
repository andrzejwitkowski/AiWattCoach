use super::checkpoint::{parse_terminal_coach_reply_checkpoint, parse_terminal_task_error};
use super::*;
use crate::domain::task_scheduler::{parse_failed_or_error_message, ResultTaskHandler};
use crate::domain::workout_summary::SaveSummaryResult;

#[derive(Clone)]
struct CoachReplyTaskResultHandler<Base> {
    base: Arc<Base>,
    user_id: String,
    workout_id: String,
}

impl<Base> ResultTaskHandler for CoachReplyTaskResultHandler<Base>
where
    Base: WorkoutSummaryUseCases + 'static,
{
    type Completed = checkpoint::CompletedCoachReplyTaskCheckpoint;
    type Output = CoachReply;
    type Error = WorkoutSummaryError;

    fn task_disappeared(&self, _task_id: &str) -> Self::Error {
        WorkoutSummaryError::Repository(
            "coach reply task disappeared before completion".to_string(),
        )
    }

    fn task_timed_out(&self, _task_id: &str) -> Self::Error {
        WorkoutSummaryError::Repository("coach reply task timed out".to_string())
    }

    fn parse_completed(&self, task: &ScheduledTask) -> Result<Self::Completed, Self::Error> {
        parse_terminal_coach_reply_checkpoint(task)?.ok_or_else(|| {
            WorkoutSummaryError::Repository(
                "completed coach reply task missing persisted checkpoint".to_string(),
            )
        })
    }

    fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
        Ok(parse_failed_or_error_message(
            parse_terminal_task_error(task)?,
            task.error_message.clone(),
            "coach reply task failed without an error message",
            WorkoutSummaryError::Repository,
        ))
    }

    fn finish(&self, completed: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        let user_id = self.user_id.clone();
        let workout_id = self.workout_id.clone();
        Box::pin(async move {
            let summary = base.get_summary(&user_id, &workout_id).await?;
            Ok(CoachReply {
                summary,
                coach_message: completed.coach_message,
                athlete_summary_was_regenerated: completed.athlete_summary_was_regenerated,
            })
        })
    }
}

pub struct SchedulerBackedWorkoutSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    base: Arc<Base>,
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    ids: Ids,
}

impl<Base, Tasks, Workers, Time, Ids> Clone
    for SchedulerBackedWorkoutSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: Clone,
{
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            scheduler: self.scheduler.clone(),
            ids: self.ids.clone(),
        }
    }
}

impl<Base, Tasks, Workers, Time, Ids>
    SchedulerBackedWorkoutSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Base: WorkoutSummaryUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        base: Arc<Base>,
        scheduler: TaskSchedulerService<Tasks, Workers, Time>,
        ids: Ids,
    ) -> Self {
        Self {
            base,
            scheduler,
            ids,
        }
    }

    fn build_coach_reply_task(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> Result<ScheduledTask, WorkoutSummaryError> {
        build_scheduled_task(NewScheduledTaskInput {
            id: self.ids.new_id("task"),
            user_id: user_id.to_string(),
            task_type: COACH_REPLY_TASK_TYPE,
            payload: WorkoutSummaryCoachReplyTaskPayload {
                user_id: user_id.to_string(),
                workout_id: workout_id.to_string(),
                user_message_id: user_message_id.to_string(),
            },
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 3,
                delay_seconds: 30,
            },
            dedupe_key: coach_reply_dedupe_key(user_id, workout_id, user_message_id),
            execution_timeout_seconds: COACH_REPLY_EXECUTION_TIMEOUT_SECONDS,
            leader_only: false,
            now_epoch_seconds: self.scheduler.now_epoch_seconds(),
        })
        .map_err(map_build_scheduled_task_error)
    }

    async fn wait_for_coach_reply_result(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        let task = self.build_coach_reply_task(user_id, workout_id, user_message_id)?;
        self.scheduler
            .enqueue_result_task(
                task,
                map_task_scheduler_error,
                CoachReplyTaskResultHandler {
                    base: self.base.clone(),
                    user_id: user_id.to_string(),
                    workout_id: workout_id.to_string(),
                },
            )
            .await
    }
}

fn map_build_scheduled_task_error(error: BuildScheduledTaskError) -> WorkoutSummaryError {
    match error {
        BuildScheduledTaskError::SerializePayload(error) => WorkoutSummaryError::Repository(
            format!("failed to serialize workout summary coach reply task payload: {error}"),
        ),
        BuildScheduledTaskError::Scheduler(error) => map_task_scheduler_error(error),
    }
}

impl<Base, Tasks, Workers, Time, Ids> WorkoutSummaryUseCases
    for SchedulerBackedWorkoutSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Base: WorkoutSummaryUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        self.base.get_summary(user_id, workout_id)
    }

    fn create_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        self.base.create_summary(user_id, workout_id)
    }

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        self.base.list_summaries(user_id, workout_ids)
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        self.base.update_rpe(user_id, workout_id, rpe)
    }

    fn mark_saved(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>> {
        self.base.mark_saved(user_id, workout_id)
    }

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        self.base.reopen_summary(user_id, workout_id)
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: crate::domain::workout_summary::WorkoutRecap,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        self.base.persist_workout_recap(user_id, workout_id, recap)
    }

    fn send_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let persisted = service
                .base
                .append_user_message(&user_id, &workout_id, content)
                .await?;
            let reply = service
                .wait_for_coach_reply_result(&user_id, &workout_id, &persisted.user_message.id)
                .await?;

            Ok(SendMessageResult {
                summary: reply.summary,
                user_message: persisted.user_message,
                coach_message: reply.coach_message,
            })
        })
    }

    fn append_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
        self.base.append_user_message(user_id, workout_id, content)
    }

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            service
                .wait_for_coach_reply_result(&user_id, &workout_id, &user_message_id)
                .await
        })
    }
}
