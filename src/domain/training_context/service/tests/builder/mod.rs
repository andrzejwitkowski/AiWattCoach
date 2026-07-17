use crate::domain::external_sync::{
    CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalProvider, ExternalSyncState,
    ExternalSyncStateRepository, ExternalSyncStatus,
};

mod focus_and_aliases;
mod load_history;
mod preview_aligned_seed;
mod rendering;

#[derive(Clone, Default)]
struct TestSyncStates {
    states: Vec<ExternalSyncState>,
}

impl ExternalSyncStateRepository for TestSyncStates {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, crate::domain::external_sync::ExternalSyncRepositoryError>,
    > {
        Box::pin(async move { Ok(state) })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, crate::domain::external_sync::ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(states
                .into_iter()
                .filter(|state| state.user_id == user_id)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .collect())
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<
            Option<ExternalSyncState>,
            crate::domain::external_sync::ExternalSyncRepositoryError,
        >,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, crate::domain::external_sync::ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(states
                .into_iter()
                .filter(|state| state.user_id == user_id)
                .filter(|state| state.provider == provider)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .collect())
        })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<(), crate::domain::external_sync::ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(()) })
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<
            Option<ExternalSyncState>,
            crate::domain::external_sync::ExternalSyncRepositoryError,
        >,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(states.into_iter().find(|state| {
                state.user_id == user_id
                    && state.provider == ExternalProvider::Wahoo
                    && state.wahoo_plan_id == Some(wahoo_plan_id)
            }))
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<
            Option<ExternalSyncState>,
            crate::domain::external_sync::ExternalSyncRepositoryError,
        >,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            Ok(states.into_iter().find(|state| {
                state.user_id == user_id
                    && state.provider == ExternalProvider::Wahoo
                    && state.wahoo_workout_token.as_deref() == Some(wahoo_workout_token.as_str())
            }))
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<
            Option<ExternalSyncState>,
            crate::domain::external_sync::ExternalSyncRepositoryError,
        >,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(states.into_iter().find(|state| {
                state.user_id == user_id
                    && state.provider == provider
                    && state.external_id.as_deref() == Some(external_id.as_str())
            }))
        })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<
            Option<ExternalSyncState>,
            crate::domain::external_sync::ExternalSyncRepositoryError,
        >,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(states.into_iter().find(|state| {
                state.user_id == user_id
                    && state.provider == provider
                    && state.canonical_entity.entity_kind == CanonicalEntityKind::PlannedWorkout
                    && state.external_id.as_deref() == Some(external_id.as_str())
            }))
        })
    }
}

fn wahoo_sync_state(id: &str) -> ExternalSyncState {
    ExternalSyncState {
        user_id: "user-1".to_string(),
        provider: ExternalProvider::Wahoo,
        canonical_entity: CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            id.to_string(),
        ),
        external_id: Some(id.to_string()),
        wahoo_plan_external_id: None,
        wahoo_plan_id: None,
        wahoo_workout_id: None,
        wahoo_workout_token: None,
        sync_status: ExternalSyncStatus::Synced,
        last_synced_payload_hash: None,
        last_seen_remote_payload_hash: None,
        last_error: None,
        last_synced_at_epoch_seconds: None,
        last_seen_remote_at_epoch_seconds: None,
        conflict_status: ConflictStatus::InSync,
    }
}
