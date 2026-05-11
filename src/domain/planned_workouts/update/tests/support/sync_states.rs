use std::sync::{Arc, Mutex};

use crate::domain::external_sync::{
    BoxFuture, CanonicalEntityKind, CanonicalEntityRef, ExternalProvider,
    ExternalSyncRepositoryError, ExternalSyncState, ExternalSyncStateRepository,
};

#[derive(Clone, Default)]
pub struct InMemoryExternalSyncStateRepository {
    stored: Arc<Mutex<Vec<ExternalSyncState>>>,
    operation_log: Arc<Mutex<Vec<String>>>,
    shared_log: Option<Arc<Mutex<Vec<String>>>>,
}

impl InMemoryExternalSyncStateRepository {
    pub fn with_states_and_shared_log(
        states: Vec<ExternalSyncState>,
        shared_log: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            stored: Arc::new(Mutex::new(states)),
            operation_log: Arc::new(Mutex::new(Vec::new())),
            shared_log: Some(shared_log),
        }
    }

    pub fn stored(&self) -> Vec<ExternalSyncState> {
        self.stored
            .lock()
            .expect("sync states mutex poisoned")
            .clone()
    }
}

impl ExternalSyncStateRepository for InMemoryExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> BoxFuture<Result<ExternalSyncState, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let operation_log = self.operation_log.clone();
        let shared_log = self.shared_log.clone();
        Box::pin(async move {
            let entry = format!("sync_states.upsert:{}", state.sync_status.as_str());
            operation_log
                .lock()
                .expect("sync states mutex poisoned")
                .push(entry.clone());
            if let Some(shared_log) = shared_log {
                shared_log
                    .lock()
                    .expect("shared log mutex poisoned")
                    .push(entry);
            }
            let mut stored = stored.lock().expect("sync states mutex poisoned");
            stored.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.canonical_entity == state.canonical_entity)
            });
            stored.push(state.clone());
            Ok(state)
        })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> BoxFuture<Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .filter(|state| state.user_id == user_id)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity == canonical_entity
                })
                .cloned())
        })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entities: &[CanonicalEntityRef],
    ) -> BoxFuture<Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .filter(|state| state.user_id == user_id && state.provider == provider)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .cloned()
                .collect())
        })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            stored
                .lock()
                .expect("sync states mutex poisoned")
                .retain(|state| {
                    !(state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity == canonical_entity)
                });
            Ok(())
        })
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_plan_id == Some(wahoo_plan_id)
                })
                .cloned())
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_workout_token.as_deref()
                            == Some(wahoo_workout_token.as_str())
                })
                .cloned())
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("sync states mutex poisoned")
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity.entity_kind == CanonicalEntityKind::PlannedWorkout
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }
}
