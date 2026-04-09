use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use aiwattcoach::domain::{
    llm::BoxFuture,
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, Weekday,
    },
    workout_summary::{
        validate_message_content, CoachReply, CoachReplyClaimResult, CoachReplyOperation,
        CoachReplyOperationRepository, MessageRole, PersistedUserMessage, SaveSummaryResult,
        SaveWorkflowResult, SaveWorkflowStatus, SendMessageResult, WorkoutRecap, WorkoutSummary,
        WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryUseCases,
    },
};

type CoachReplyOperationKey = (String, String, String);
type CoachReplyOperationStore = BTreeMap<CoachReplyOperationKey, CoachReplyOperation>;

#[derive(Clone, Default)]
pub(crate) struct TestWorkoutSummaryService {
    summaries: Arc<Mutex<Vec<WorkoutSummary>>>,
    processed_user_messages: Arc<Mutex<Vec<String>>>,
    coach_reply_delay: Option<Duration>,
    availability_configured: bool,
}

impl TestWorkoutSummaryService {
    pub(crate) fn with_summaries(summaries: Vec<WorkoutSummary>) -> Self {
        Self {
            summaries: Arc::new(Mutex::new(summaries)),
            processed_user_messages: Arc::new(Mutex::new(Vec::new())),
            coach_reply_delay: None,
            availability_configured: true,
        }
    }

    pub(crate) fn with_coach_reply_delay(mut self, delay: Duration) -> Self {
        self.coach_reply_delay = Some(delay);
        self
    }

    pub(crate) fn with_availability_configured(mut self, configured: bool) -> Self {
        self.availability_configured = configured;
        self
    }

    pub(crate) fn processed_user_messages(&self) -> Vec<String> {
        self.processed_user_messages.lock().unwrap().clone()
    }

    pub(crate) fn summary(&self, user_id: &str, workout_id: &str) -> Option<WorkoutSummary> {
        self.summaries
            .lock()
            .unwrap()
            .iter()
            .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            .cloned()
    }
}

impl WorkoutSummaryUseCases for TestWorkoutSummaryService {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            summaries
                .lock()
                .unwrap()
                .iter()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
                .cloned()
                .ok_or(WorkoutSummaryError::NotFound)
        })
    }

    fn create_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            if let Some(existing) = summaries
                .iter()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
                .cloned()
            {
                return Ok(existing);
            }

            let summary = sample_summary_for_user(&user_id, &workout_id);
            summaries.push(summary.clone());
            Ok(summary)
        })
    }

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut filtered: Vec<_> = summaries
                .lock()
                .unwrap()
                .iter()
                .filter(|summary| {
                    summary.user_id == user_id
                        && workout_ids.iter().any(|id| id == &summary.workout_id)
                })
                .cloned()
                .collect();
            filtered.sort_by(|left, right| {
                right
                    .updated_at_epoch_seconds
                    .cmp(&left.updated_at_epoch_seconds)
                    .then_with(|| {
                        right
                            .created_at_epoch_seconds
                            .cmp(&left.created_at_epoch_seconds)
                    })
            });
            Ok(filtered)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }

            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(summary.clone())
        })
    }

    fn mark_saved(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before saving workout summary".to_string(),
                ));
            }

            summary.saved_at_epoch_seconds = Some(1_700_000_100);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(SaveSummaryResult {
                summary: summary.clone(),
                workflow: SaveWorkflowResult {
                    recap_status: SaveWorkflowStatus::Skipped,
                    plan_status: SaveWorkflowStatus::Skipped,
                    messages: Vec::new(),
                },
            })
        })
    }

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            summary.saved_at_epoch_seconds = None;
            summary.updated_at_epoch_seconds = 1_700_000_300;
            Ok(summary.clone())
        })
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            summary.workout_recap_text = Some(recap.text);
            summary.workout_recap_provider = Some(recap.provider);
            summary.workout_recap_model = Some(recap.model);
            summary.workout_recap_generated_at_epoch_seconds =
                Some(recap.generated_at_epoch_seconds);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(summary.clone())
        })
    }

    fn send_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let processed_user_messages = self.processed_user_messages.clone();
        let availability_configured = self.availability_configured;
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let content = validate_message_content(&content)?;
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }
            if !availability_configured {
                return Err(WorkoutSummaryError::Validation(
                    "availability must be configured before chatting with coach".to_string(),
                ));
            }

            processed_user_messages
                .lock()
                .unwrap()
                .push(content.clone());

            let next_user_suffix = summary.messages.len() + 1;
            let user_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-user-{next_user_suffix}"),
                role: MessageRole::User,
                content,
                created_at_epoch_seconds: 1_700_000_000,
            };
            let coach_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-coach-{}", next_user_suffix + 1),
                role: MessageRole::Coach,
                content: "Thanks, that helps. What stood out most about how the workout felt compared with the plan?".to_string(),
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(user_message.clone());
            summary.messages.push(coach_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(SendMessageResult {
                summary: summary.clone(),
                user_message,
                coach_message,
            })
        })
    }

    fn append_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let processed_user_messages = self.processed_user_messages.clone();
        let availability_configured = self.availability_configured;
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let content = validate_message_content(&content)?;
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }
            if !availability_configured {
                return Err(WorkoutSummaryError::Validation(
                    "availability must be configured before chatting with coach".to_string(),
                ));
            }

            processed_user_messages
                .lock()
                .unwrap()
                .push(content.clone());

            let next_user_suffix = summary.messages.len() + 1;
            let user_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-user-{next_user_suffix}"),
                role: MessageRole::User,
                content,
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(user_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(PersistedUserMessage {
                summary: summary.clone(),
                user_message,
                athlete_summary_may_regenerate_before_reply: false,
            })
        })
    }

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let coach_reply_delay = self.coach_reply_delay;
        let availability_configured = self.availability_configured;
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            if let Some(delay) = coach_reply_delay {
                tokio::time::sleep(delay).await;
            }

            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }
            if !availability_configured {
                return Err(WorkoutSummaryError::Validation(
                    "availability must be configured before chatting with coach".to_string(),
                ));
            }

            let user_message_content = summary
                .messages
                .iter()
                .find(|message| message.id == user_message_id && message.role == MessageRole::User)
                .map(|message| message.content.clone())
                .ok_or_else(|| {
                    WorkoutSummaryError::Validation(
                        "user message must be persisted before generating coach reply".to_string(),
                    )
                })?;

            let next_coach_suffix = summary.messages.len() + 1;
            let coach_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-coach-{next_coach_suffix}"),
                role: MessageRole::Coach,
                content: format!("Coach reply to: {user_message_content}"),
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(coach_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(CoachReply {
                summary: summary.clone(),
                coach_message,
                athlete_summary_was_regenerated: false,
            })
        })
    }
}

pub(crate) fn sample_summary(workout_id: &str) -> WorkoutSummary {
    sample_summary_for_user("user-1", workout_id)
}

fn sample_summary_for_user(user_id: &str, workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryWorkoutSummaryRepository {
    summaries: Arc<Mutex<BTreeMap<(String, String), WorkoutSummary>>>,
}

impl InMemoryWorkoutSummaryRepository {
    pub(crate) fn with_summary(summary: WorkoutSummary) -> Self {
        let mut summaries = BTreeMap::new();
        summaries.insert(
            (summary.user_id.clone(), summary.workout_id.clone()),
            summary,
        );

        Self {
            summaries: Arc::new(Mutex::new(summaries)),
        }
    }
}

impl WorkoutSummaryRepository for InMemoryWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            Ok(summaries
                .lock()
                .unwrap()
                .get(&(user_id, workout_id))
                .cloned())
        })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let summaries = summaries.lock().unwrap();
            Ok(workout_ids
                .into_iter()
                .filter_map(|workout_id| summaries.get(&(user_id.clone(), workout_id)).cloned())
                .collect())
        })
    }

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let key = (summary.user_id.clone(), summary.workout_id.clone());
            let mut summaries = summaries.lock().unwrap();
            if summaries.contains_key(&key) {
                return Err(WorkoutSummaryError::AlreadyExists);
            }
            summaries.insert(key, summary.clone());
            Ok(summary)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn set_saved_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: Option<i64>,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.saved_at_epoch_seconds = saved_at_epoch_seconds;
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.workout_recap_text = Some(recap.text);
            summary.workout_recap_provider = Some(recap.provider);
            summary.workout_recap_model = Some(recap.model);
            summary.workout_recap_generated_at_epoch_seconds =
                Some(recap.generated_at_epoch_seconds);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn append_message(
        &self,
        user_id: &str,
        workout_id: &str,
        message: aiwattcoach::domain::workout_summary::ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if summary
                .messages
                .iter()
                .any(|existing| existing.id == message.id)
            {
                return Ok(());
            }
            summary.messages.push(message);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> BoxFuture<
        Result<
            Option<aiwattcoach::domain::workout_summary::ConversationMessage>,
            WorkoutSummaryError,
        >,
    > {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let message_id = message_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            Ok(summaries
                .lock()
                .unwrap()
                .get(&(user_id, workout_id))
                .and_then(|summary| {
                    summary
                        .messages
                        .iter()
                        .rev()
                        .find(|message| message.id == message_id)
                        .cloned()
                }))
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryCoachReplyOperationRepository {
    operations: Arc<Mutex<CoachReplyOperationStore>>,
}

impl CoachReplyOperationRepository for InMemoryCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let key = (
            user_id.to_string(),
            workout_id.to_string(),
            user_message_id.to_string(),
        );
        let operations = self.operations.clone();
        Box::pin(async move { Ok(operations.lock().unwrap().get(&key).cloned()) })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        _stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let key = (
            operation.user_id.clone(),
            operation.workout_id.clone(),
            operation.user_message_id.clone(),
        );
        let operations = self.operations.clone();
        Box::pin(async move {
            let mut operations = operations.lock().unwrap();
            if let Some(existing) = operations.get(&key).cloned() {
                return Ok(CoachReplyClaimResult::Existing(existing));
            }
            operations.insert(key, operation.clone());
            Ok(CoachReplyClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let key = (
            operation.user_id.clone(),
            operation.workout_id.clone(),
            operation.user_message_id.clone(),
        );
        let operations = self.operations.clone();
        Box::pin(async move {
            operations.lock().unwrap().insert(key, operation.clone());
            Ok(operation)
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestAvailabilitySettingsService {
    configured: bool,
}

impl TestAvailabilitySettingsService {
    pub(crate) fn unconfigured() -> Arc<dyn UserSettingsUseCases> {
        Arc::new(Self { configured: false })
    }
}

impl UserSettingsUseCases for TestAvailabilitySettingsService {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let user_id = user_id.to_string();
        let configured = self.configured;
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1_700_000_000);
            if configured {
                settings.availability = AvailabilitySettings {
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
            }
            Ok(Some(settings))
        })
    }

    fn get_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        let configured = self.configured;
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1_700_000_000);
            if configured {
                settings.availability = AvailabilitySettings {
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
            }
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
}

#[derive(Clone)]
pub(crate) struct TestClock;

impl aiwattcoach::domain::identity::Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestIdGenerator;

impl aiwattcoach::domain::identity::IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-1")
    }
}

pub(crate) fn existing_summary() -> WorkoutSummary {
    sample_summary("workout-1")
}

pub(crate) fn sample_summary_with_updated_at(
    workout_id: &str,
    updated_at_epoch_seconds: i64,
) -> WorkoutSummary {
    let mut summary = sample_summary(workout_id);
    summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
    summary
}
