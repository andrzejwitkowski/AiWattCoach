use super::*;

#[derive(Clone, Default)]
pub(crate) struct InMemoryWorkoutSummaryRepository {
    summaries: Arc<Mutex<BTreeMap<(String, String), WorkoutSummary>>>,
    calls: Arc<Mutex<Vec<String>>>,
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
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
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
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(format!("create:{}", summary.workout_id));
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
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(format!("update_rpe:{workout_id}"));
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
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "set_saved_state:{workout_id}:{saved_at_epoch_seconds:?}"
            ));
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
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(format!("persist_workout_recap:{workout_id}"));
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
        message: ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "append_message:{}:{}",
                workout_id,
                match message.role {
                    MessageRole::User => "user",
                    MessageRole::Coach => "coach",
                }
            ));
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
    ) -> BoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
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
