use tracing::warn;

use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    external_sync::{
        CanonicalEntityRef, ExternalObjectKind, ExternalProvider, ExternalSyncRepositoryError,
    },
};

use super::{ExternalImportError, ExternalImportOutcome, SyncMetadataInput};

pub(super) async fn finalize_import<Refresh, Persist>(
    refresh: &Refresh,
    persist_sync_metadata: Persist,
    user_id: &str,
    import_date: &str,
    provider: ExternalProvider,
    external_id: String,
    canonical_entity: CanonicalEntityRef,
) -> Result<ExternalImportOutcome, ExternalImportError>
where
    Refresh: CalendarEntryViewRefreshPort,
    Persist: std::future::Future<Output = Result<(), ExternalImportError>>,
{
    persist_sync_metadata.await?;
    refresh_imported_range(refresh, user_id, import_date, import_date).await;

    Ok(ExternalImportOutcome {
        canonical_entity,
        provider,
        external_id,
    })
}

pub(super) async fn refresh_imported_range<Refresh>(
    refresh: &Refresh,
    user_id: &str,
    oldest: &str,
    newest: &str,
) where
    Refresh: CalendarEntryViewRefreshPort,
{
    if let Err(error) = refresh
        .refresh_range_for_user(user_id, oldest, newest)
        .await
    {
        warn!(
            %user_id,
            %oldest,
            %newest,
            %error,
            "external import succeeded but calendar view refresh failed"
        );
    }
}

pub(super) fn sync_metadata_input(
    provider: ExternalProvider,
    external_object_kind: ExternalObjectKind,
    external_id: String,
    canonical_entity: CanonicalEntityRef,
    normalized_payload_hash: String,
    dedup_key: Option<String>,
) -> SyncMetadataInput {
    SyncMetadataInput {
        provider,
        external_object_kind,
        external_id,
        canonical_entity,
        normalized_payload_hash,
        dedup_key,
    }
}

pub(super) fn map_repository_error(error: ExternalSyncRepositoryError) -> ExternalImportError {
    match error {
        ExternalSyncRepositoryError::Storage(message)
        | ExternalSyncRepositoryError::CorruptData(message) => {
            ExternalImportError::Repository(message)
        }
    }
}
