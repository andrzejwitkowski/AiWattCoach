use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

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

    pub fn register(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> watch::Receiver<Option<SaveWorkflowDto>> {
        let (tx, rx) = watch::channel(None);
        self.channels
            .lock()
            .unwrap()
            .insert(Self::key(user_id, workout_id), tx);
        rx
    }

    pub fn subscribe(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Option<watch::Receiver<Option<SaveWorkflowDto>>> {
        let key = Self::key(user_id, workout_id);
        self.channels
            .lock()
            .unwrap()
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
        let mut channels = self.channels.lock().unwrap();
        if let Some(tx) = channels.get(&key) {
            let payload = SaveWorkflowDto {
                recap_status: map_workflow_status_to_dto(recap_status),
                plan_status: map_workflow_status_to_dto(plan_status),
                messages,
            };
            if tx.send(Some(payload)).is_err() {
                channels.remove(&key);
            }
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
