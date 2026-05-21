use std::sync::{Arc, Mutex};

use crate::domain::external_sync::{
    CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError, ExternalSyncState,
    ExternalSyncStateRepository,
};

#[derive(Clone, Default)]
pub(crate) struct FixedSyncStateRepository {
    states: Arc<Mutex<Vec<ExternalSyncState>>>,
}

impl FixedSyncStateRepository {
    pub(crate) fn seed_state(&self, state: ExternalSyncState) {
        self.states
            .lock()
            .expect("sync state mutex poisoned")
            .push(state);
    }
}

impl ExternalSyncStateRepository for FixedSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        Box::pin(async move {
            let mut states = states.lock().expect("sync state mutex poisoned");
            states.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.canonical_entity == state.canonical_entity)
            });
            states.push(state.clone());
            Ok(state)
        })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let entity_ids = canonical_entities
            .iter()
            .map(|entity| entity.entity_id.clone())
            .collect::<std::collections::HashSet<_>>();
        Box::pin(async move {
            Ok(states
                .lock()
                .expect("sync state mutex poisoned")
                .iter()
                .filter(|state| {
                    state.user_id == user_id
                        && entity_ids.contains(&state.canonical_entity.entity_id)
                })
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        Box::pin(async { Ok(()) })
    }

    fn find_by_wahoo_plan_id(
        &self,
        _user_id: &str,
        _wahoo_plan_id: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_wahoo_workout_token(
        &self,
        _user_id: &str,
        _wahoo_workout_token: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_provider_and_external_id(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }
}
