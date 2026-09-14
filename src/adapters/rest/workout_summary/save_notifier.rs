use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{broadcast, watch};
use tracing::warn;

use crate::domain::workout_summary::SaveWorkflowStatus;

use super::dto::SaveWorkflowDto;
use super::mapping::map_workflow_status_to_dto;

#[derive(Clone)]
struct SaveWorkflowChannels {
    completion: watch::Sender<Option<SaveWorkflowDto>>,
    progress: broadcast::Sender<String>,
}

#[derive(Clone)]
pub struct WorkoutSummarySaveNotifier {
    channels: Arc<Mutex<HashMap<String, SaveWorkflowChannels>>>,
}

impl Default for WorkoutSummarySaveNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkoutSummarySaveNotifier {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn key(user_id: &str, workout_id: &str) -> String {
        format!("{user_id}:{workout_id}")
    }

    fn channels(&self, operation: &str) -> MutexGuard<'_, HashMap<String, SaveWorkflowChannels>> {
        self.channels.lock().unwrap_or_else(|error| {
            warn!(
                operation = %operation,
                error = %error,
                "Workout summary save notifier lock was poisoned; recovering state"
            );
            error.into_inner()
        })
    }

    fn new_channels() -> (
        SaveWorkflowChannels,
        watch::Receiver<Option<SaveWorkflowDto>>,
    ) {
        let (completion_tx, completion_rx) = watch::channel(None);
        let (progress_tx, _) = broadcast::channel(32);
        (
            SaveWorkflowChannels {
                completion: completion_tx,
                progress: progress_tx,
            },
            completion_rx,
        )
    }

    pub fn register(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> (
        watch::Receiver<Option<SaveWorkflowDto>>,
        broadcast::Receiver<String>,
    ) {
        let mut channels = self.channels("register");
        match channels.entry(Self::key(user_id, workout_id)) {
            Entry::Occupied(entry) => {
                let channels = entry.get();
                (
                    channels.completion.subscribe(),
                    channels.progress.subscribe(),
                )
            }
            Entry::Vacant(entry) => {
                let (channels, completion_rx) = Self::new_channels();
                let progress_rx = channels.progress.subscribe();
                entry.insert(channels);
                (completion_rx, progress_rx)
            }
        }
    }

    pub fn unregister(&self, user_id: &str, workout_id: &str) {
        self.channels("unregister")
            .remove(&Self::key(user_id, workout_id));
    }

    pub fn subscribe(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Option<(
        watch::Receiver<Option<SaveWorkflowDto>>,
        broadcast::Receiver<String>,
    )> {
        let key = Self::key(user_id, workout_id);
        self.channels("subscribe").get(&key).map(|channels| {
            (
                channels.completion.subscribe(),
                channels.progress.subscribe(),
            )
        })
    }

    pub fn send(
        &self,
        user_id: &str,
        workout_id: &str,
        recap_status: SaveWorkflowStatus,
        plan_status: SaveWorkflowStatus,
        messages: Vec<String>,
    ) {
        let key = Self::key(user_id, workout_id);
        let tx = self
            .channels("send")
            .get(&key)
            .map(|c| c.completion.clone());
        if let Some(tx) = tx {
            let payload = SaveWorkflowDto {
                recap_status: map_workflow_status_to_dto(recap_status),
                plan_status: map_workflow_status_to_dto(plan_status),
                messages,
            };
            tx.send_replace(Some(payload));
        }
    }

    pub fn send_progress(&self, user_id: &str, workout_id: &str, message: String) {
        let key = Self::key(user_id, workout_id);
        let tx = self
            .channels("send_progress")
            .get(&key)
            .map(|c| c.progress.clone());
        if let Some(tx) = tx {
            let _ = tx.send(message);
        }
    }
}

impl crate::domain::workout_summary::SaveWorkflowCompletionPort for WorkoutSummarySaveNotifier {
    fn on_completed(
        &self,
        user_id: &str,
        workout_id: &str,
        recap_status: SaveWorkflowStatus,
        plan_status: SaveWorkflowStatus,
        messages: Vec<String>,
    ) {
        self.send(user_id, workout_id, recap_status, plan_status, messages);
    }
}

impl crate::domain::training_plan::PlanQualityProgressPort for WorkoutSummarySaveNotifier {
    fn on_quality_progress(&self, user_id: &str, workout_id: &str, message: String) {
        self.send_progress(user_id, workout_id, message);
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::workout_summary::SaveWorkflowStatus;

    use super::WorkoutSummarySaveNotifier;

    #[test]
    fn send_stores_completion_for_late_subscribers() {
        let notifier = WorkoutSummarySaveNotifier::new();
        let _registered = notifier.register("user-1", "workout-1");

        notifier.send(
            "user-1",
            "workout-1",
            SaveWorkflowStatus::Generated,
            SaveWorkflowStatus::Generated,
            vec!["done".to_string()],
        );

        let (received, _) = notifier.subscribe("user-1", "workout-1").unwrap();
        assert!(received.borrow().is_some());
    }

    #[test]
    fn register_reuses_existing_sender_for_current_subscribers() {
        let notifier = WorkoutSummarySaveNotifier::new();
        let (current, _) = notifier.register("user-1", "workout-1");
        let _second_registration = notifier.register("user-1", "workout-1");

        notifier.send(
            "user-1",
            "workout-1",
            SaveWorkflowStatus::Generated,
            SaveWorkflowStatus::Generated,
            vec!["done".to_string()],
        );

        assert!(current.borrow().is_some());
    }
}
