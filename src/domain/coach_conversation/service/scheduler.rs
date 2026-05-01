use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{LlmError, LLM_REQUEST_TIMEOUT_SECONDS},
    task_scheduler::{
        NewTask, ResultTaskHandler, RetryStrategy, ScheduledTask, SharedTaskHandler, TaskHandler,
        TaskRepository, TaskRunOutcome, TaskSchedulerError, TaskSchedulerService,
        TaskWorkerRepository,
    },
};

use super::{
    BoxFuture, CoachConversationError, CoachConversationMessage, CoachConversationReply,
    CoachConversationUseCases, STALE_PENDING_TIMEOUT_SECONDS,
};

pub(crate) const COACH_CONVERSATION_REPLY_TASK_TYPE: &str = "coach_conversation.reply";
pub(crate) const COACH_CONVERSATION_REPLY_EXECUTION_TIMEOUT_BUFFER_SECONDS: i64 = 30;
pub(crate) const COACH_CONVERSATION_REPLY_EXECUTION_TIMEOUT_SECONDS: i64 =
    (LLM_REQUEST_TIMEOUT_SECONDS as i64 * 2)
        + COACH_CONVERSATION_REPLY_EXECUTION_TIMEOUT_BUFFER_SECONDS;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CoachConversationReplyTaskPayload {
    user_id: String,
    conversation_id: String,
    user_message_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompletedCoachConversationReplyTaskCheckpoint {
    coach_message: CoachConversationMessage,
    athlete_summary_was_regenerated: bool,
}

fn coach_conversation_reply_dedupe_key(
    user_id: &str,
    conversation_id: &str,
    user_message_id: &str,
) -> String {
    format!("coach-conversation:{user_id}:{conversation_id}:{user_message_id}")
}

fn parse_task_payload(
    task: &ScheduledTask,
) -> Result<CoachConversationReplyTaskPayload, CoachConversationError> {
    serde_json::from_value(task.payload.clone()).map_err(|error| {
        CoachConversationError::Repository(format!(
            "invalid coach conversation reply task payload: {error}"
        ))
    })
}

fn map_task_scheduler_error(error: TaskSchedulerError) -> CoachConversationError {
    match error {
        TaskSchedulerError::Validation(message)
        | TaskSchedulerError::Conflict(message)
        | TaskSchedulerError::Repository(message) => CoachConversationError::Repository(message),
    }
}

fn llm_error_is_retryable(error: &LlmError) -> bool {
    error.is_retryable()
}

#[derive(Clone)]
struct CoachConversationTaskResultHandler<Base> {
    base: Arc<Base>,
    user_id: String,
    conversation_id: String,
}

impl<Base> ResultTaskHandler for CoachConversationTaskResultHandler<Base>
where
    Base: CoachConversationUseCases + 'static,
{
    type Completed = CompletedCoachConversationReplyTaskCheckpoint;
    type Output = CoachConversationReply;
    type Error = CoachConversationError;

    fn task_disappeared(&self, _task_id: &str) -> Self::Error {
        CoachConversationError::Repository(
            "coach conversation reply task disappeared before completion".to_string(),
        )
    }

    fn task_timed_out(&self, _task_id: &str) -> Self::Error {
        CoachConversationError::Repository("coach conversation reply task timed out".to_string())
    }

    fn parse_completed(&self, task: &ScheduledTask) -> Result<Self::Completed, Self::Error> {
        task.checkpoint
            .clone()
            .ok_or_else(|| {
                CoachConversationError::Repository(
                    "completed coach conversation reply task missing persisted checkpoint"
                        .to_string(),
                )
            })
            .and_then(|value| {
                serde_json::from_value(value).map_err(|error| {
                    CoachConversationError::Repository(format!(
                        "invalid completed coach conversation reply task checkpoint: {error}"
                    ))
                })
            })
    }

    fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
        Ok(task
            .error_message
            .clone()
            .map(CoachConversationError::Repository)
            .unwrap_or_else(|| {
                CoachConversationError::Repository(
                    "coach conversation reply task failed without an error message".to_string(),
                )
            }))
    }

    fn finish(&self, completed: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        let user_id = self.user_id.clone();
        let conversation_id = self.conversation_id.clone();
        Box::pin(async move {
            let (conversation, messages) = base
                .get_calendar_conversation(&user_id, &conversation_id)
                .await?;
            Ok(CoachConversationReply {
                conversation,
                messages,
                coach_message: completed.coach_message,
                athlete_summary_was_regenerated: completed.athlete_summary_was_regenerated,
            })
        })
    }
}

pub struct SchedulerBackedCoachConversationService<Base, Tasks, Workers, Time, Ids>
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
    for SchedulerBackedCoachConversationService<Base, Tasks, Workers, Time, Ids>
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
    SchedulerBackedCoachConversationService<Base, Tasks, Workers, Time, Ids>
where
    Base: CoachConversationUseCases + 'static,
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

    fn build_reply_task(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> Result<ScheduledTask, CoachConversationError> {
        ScheduledTask::new(
            NewTask {
                id: self.ids.new_id("task"),
                user_id: user_id.to_string(),
                task_type: COACH_CONVERSATION_REPLY_TASK_TYPE.to_string(),
                payload: serde_json::to_value(CoachConversationReplyTaskPayload {
                    user_id: user_id.to_string(),
                    conversation_id: conversation_id.to_string(),
                    user_message_id: user_message_id.to_string(),
                })
                .map_err(|error| {
                    CoachConversationError::Repository(format!(
                        "failed to serialize coach conversation reply task payload: {error}"
                    ))
                })?,
                retry_strategy: RetryStrategy::Fixed {
                    max_attempts: 3,
                    delay_seconds: 30,
                },
                dedupe_key: coach_conversation_reply_dedupe_key(
                    user_id,
                    conversation_id,
                    user_message_id,
                ),
                execution_timeout_seconds: COACH_CONVERSATION_REPLY_EXECUTION_TIMEOUT_SECONDS,
                leader_only: false,
            },
            self.scheduler.now_epoch_seconds(),
        )
        .map_err(map_task_scheduler_error)
    }

    async fn wait_for_reply_result(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> Result<CoachConversationReply, CoachConversationError> {
        let task = self.build_reply_task(user_id, conversation_id, user_message_id)?;
        self.scheduler
            .enqueue_result_task(
                task,
                map_task_scheduler_error,
                CoachConversationTaskResultHandler {
                    base: self.base.clone(),
                    user_id: user_id.to_string(),
                    conversation_id: conversation_id.to_string(),
                },
            )
            .await
    }
}

impl<Base, Tasks, Workers, Time, Ids> CoachConversationUseCases
    for SchedulerBackedCoachConversationService<Base, Tasks, Workers, Time, Ids>
where
    Base: CoachConversationUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    fn get_or_create_active_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<
        Result<
            (
                crate::domain::coach_conversation::CoachConversation,
                Vec<CoachConversationMessage>,
            ),
            CoachConversationError,
        >,
    > {
        self.base
            .get_or_create_active_calendar_conversation(user_id)
    }

    fn start_new_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<
        Result<
            (
                crate::domain::coach_conversation::CoachConversation,
                Vec<CoachConversationMessage>,
            ),
            CoachConversationError,
        >,
    > {
        self.base.start_new_calendar_conversation(user_id)
    }

    fn get_calendar_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<
        Result<
            (
                crate::domain::coach_conversation::CoachConversation,
                Vec<CoachConversationMessage>,
            ),
            CoachConversationError,
        >,
    > {
        self.base
            .get_calendar_conversation(user_id, conversation_id)
    }

    fn send_calendar_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<
        Result<
            crate::domain::coach_conversation::SendConversationMessageResult,
            CoachConversationError,
        >,
    > {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let persisted = service
                .base
                .append_calendar_user_message(&user_id, &conversation_id, content)
                .await?;
            let reply = service
                .wait_for_reply_result(&user_id, &conversation_id, &persisted.user_message.id)
                .await?;
            Ok(
                crate::domain::coach_conversation::SendConversationMessageResult {
                    conversation: reply.conversation,
                    messages: reply.messages,
                    user_message: persisted.user_message,
                    coach_message: reply.coach_message,
                },
            )
        })
    }

    fn append_calendar_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<
        Result<
            crate::domain::coach_conversation::PersistedConversationUserMessage,
            CoachConversationError,
        >,
    > {
        self.base
            .append_calendar_user_message(user_id, conversation_id, content)
    }

    fn generate_calendar_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>> {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            service
                .wait_for_reply_result(&user_id, &conversation_id, &user_message_id)
                .await
        })
    }
}

struct CoachConversationReplyTaskHandler<Base> {
    base: Arc<Base>,
}

impl<Base> CoachConversationReplyTaskHandler<Base>
where
    Base: CoachConversationUseCases + 'static,
{
    fn completed_checkpoint(
        task_id: &str,
        reply: &CoachConversationReply,
    ) -> Result<serde_json::Value, CoachConversationError> {
        serde_json::to_value(CompletedCoachConversationReplyTaskCheckpoint {
            coach_message: reply.coach_message.clone(),
            athlete_summary_was_regenerated: reply.athlete_summary_was_regenerated,
        })
        .map_err(|error| {
            let error = CoachConversationError::Repository(format!(
                "failed to serialize completed coach conversation reply task checkpoint: {error}"
            ));
            warn!(task_id = %task_id, error = %error, "failed to serialize completed coach conversation reply task checkpoint");
            error
        })
    }

    fn map_task_failure(error: CoachConversationError) -> TaskRunOutcome {
        let (retryable, retry_delay_seconds) = match &error {
            CoachConversationError::Llm(llm_error) => (llm_error_is_retryable(llm_error), None),
            CoachConversationError::Repository(_) => (true, None),
            CoachConversationError::ReplyAlreadyPending => {
                (true, Some(STALE_PENDING_TIMEOUT_SECONDS))
            }
            CoachConversationError::NotFound
            | CoachConversationError::Archived
            | CoachConversationError::Validation(_) => (false, None),
        };

        TaskRunOutcome::Failed {
            checkpoint: None,
            error_message: error.to_string(),
            retryable,
            retry_delay_seconds,
        }
    }
}

impl<Base> TaskHandler for CoachConversationReplyTaskHandler<Base>
where
    Base: CoachConversationUseCases + 'static,
{
    fn task_type(&self) -> &'static str {
        COACH_CONVERSATION_REPLY_TASK_TYPE
    }

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let base = self.base.clone();
        Box::pin(async move {
            let payload = match parse_task_payload(&task) {
                Ok(payload) => payload,
                Err(error) => {
                    tracing::warn!(
                        task_id = %task.id,
                        error = %error,
                        "invalid coach conversation reply task payload"
                    );
                    return TaskRunOutcome::Failed {
                        checkpoint: None,
                        error_message: error.to_string(),
                        retryable: false,
                        retry_delay_seconds: None,
                    };
                }
            };

            match base
                .generate_calendar_reply(
                    &payload.user_id,
                    &payload.conversation_id,
                    payload.user_message_id,
                )
                .await
            {
                Ok(reply) => match Self::completed_checkpoint(&task.id, &reply) {
                    Ok(checkpoint) => TaskRunOutcome::Completed {
                        checkpoint: Some(checkpoint),
                    },
                    Err(error) => Self::map_task_failure(error),
                },
                Err(error) => Self::map_task_failure(error),
            }
        })
    }
}

pub fn coach_conversation_reply_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: CoachConversationUseCases + 'static,
{
    Arc::new(CoachConversationReplyTaskHandler { base })
}
