use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::watch;
use tracing::warn;

use crate::domain::workout_summary::SaveWorkflowStatus;

use super::dto::SaveWorkflowDto;
use super::mapping::map_workflow_status_to_dto;

#[derive(Clone)]
pub struct WorkoutSummarySaveNotifier {
    channels: Arc<Mutex<HashMap<String, watch::Sender<Option<SaveWorkflowDto>>>>>,
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

    fn channels(
        &self,
        operation: &str,
    ) -> MutexGuard<'_, HashMap<String, watch::Sender<Option<SaveWorkflowDto>>>> {
        self.channels.lock().unwrap_or_else(|error| {
            warn!(
                operation = %operation,
                error = %error,
                "Workout summary save notifier lock was poisoned; recovering state"
            );
            error.into_inner()
        })
    }

    pub fn register(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> watch::Receiver<Option<SaveWorkflowDto>> {
        let mut channels = self.channels("register");
        match channels.entry(Self::key(user_id, workout_id)) {
            Entry::Occupied(entry) => entry.get().subscribe(),
            Entry::Vacant(entry) => {
                let (tx, rx) = watch::channel(None);
                entry.insert(tx);
                rx
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
    ) -> Option<watch::Receiver<Option<SaveWorkflowDto>>> {
        let key = Self::key(user_id, workout_id);
        self.channels("subscribe")
            .get(&key)
            .map(|tx| tx.subscribe())
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
        let tx = self.channels("send").get(&key).cloned();
        if let Some(tx) = tx {
            let payload = SaveWorkflowDto {
                recap_status: map_workflow_status_to_dto(recap_status),
                plan_status: map_workflow_status_to_dto(plan_status),
                messages,
            };
            tx.send_replace(Some(payload));
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

        let received = notifier.subscribe("user-1", "workout-1").unwrap();
        assert!(received.borrow().is_some());
    }

    #[test]
    fn register_reuses_existing_sender_for_current_subscribers() {
        let notifier = WorkoutSummarySaveNotifier::new();
        let current = notifier.register("user-1", "workout-1");
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
