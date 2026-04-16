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
    refresh_dates: &[String],
    provider: ExternalProvider,
    external_id: String,
    canonical_entity: CanonicalEntityRef,
) -> Result<ExternalImportOutcome, ExternalImportError>
where
    Refresh: CalendarEntryViewRefreshPort,
    Persist: std::future::Future<Output = Result<(), ExternalImportError>>,
{
    persist_sync_metadata.await?;
    let (oldest, newest) = refresh_range_bounds(refresh_dates);
    refresh_imported_range(refresh, user_id, &oldest, &newest).await;

    Ok(ExternalImportOutcome {
        canonical_entity,
        provider,
        external_id,
    })
}

fn refresh_range_bounds(refresh_dates: &[String]) -> (String, String) {
    let mut dates = refresh_dates
        .iter()
        .filter(|date| !date.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    dates.sort();
    dates.dedup();

    match dates.as_slice() {
        [] => ("1970-01-01".to_string(), "1970-01-01".to_string()),
        [only] => (only.clone(), only.clone()),
        [first, .., last] => (first.clone(), last.clone()),
    }
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
