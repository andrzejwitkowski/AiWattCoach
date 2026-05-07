use mongodb::bson::Document;
use mongodb::Collection;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::error::is_duplicate_key_error;

pub(super) enum ClaimOutcome<Op> {
    Claimed(Op),
    Existing(Op),
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn mongo_claim_pending<Doc, Op>(
    collection: &Collection<Doc>,
    document: Doc,
    operation: Op,
    stale_before_epoch_seconds: i64,
    unique_filter: impl Fn() -> Document,
    map_document_to_operation: impl Fn(Doc) -> Result<Op, String>,
    is_reclaimable: impl Fn(&Op, i64) -> bool,
    attempt_count: impl Fn(&Op) -> i64,
    updated_at: impl Fn(&Op) -> i64,
    last_attempt_at: impl Fn(&Op) -> i64,
    build_reclaimed: impl FnOnce(&Op, &Op, i64) -> Result<(Op, Doc), String>,
) -> Result<ClaimOutcome<Op>, String>
where
    Doc: Serialize + DeserializeOwned + Unpin + Send + Sync,
{
    let inserted = collection
        .insert_one(&document)
        .await
        .map(|_| true)
        .or_else(|error| {
            if is_duplicate_key_error(&error) {
                Ok(false)
            } else {
                Err(error.to_string())
            }
        })?;

    if inserted {
        return Ok(ClaimOutcome::Claimed(operation));
    }

    let existing_document = collection
        .find_one(unique_filter())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "claimed operation disappeared before reload".to_string())?;

    let existing = map_document_to_operation(existing_document)?;

    if !is_reclaimable(&existing, stale_before_epoch_seconds) {
        return Ok(ClaimOutcome::Existing(existing));
    }

    let pending_last_attempt = last_attempt_at(&operation);
    let (reclaimed_op, reclaimed_document) =
        build_reclaimed(&existing, &operation, pending_last_attempt)?;

    let mut cas_filter = unique_filter();
    cas_filter.insert("attempt_count", attempt_count(&existing));
    cas_filter.insert("updated_at_epoch_seconds", updated_at(&existing));

    let replaced = collection
        .find_one_and_replace(cas_filter, &reclaimed_document)
        .await
        .map_err(|error| error.to_string())?;

    if replaced.is_some() {
        return Ok(ClaimOutcome::Claimed(reclaimed_op));
    }

    let latest_document = collection
        .find_one(unique_filter())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "reclaimed operation disappeared before reload".to_string())?;

    let latest = map_document_to_operation(latest_document)?;
    Ok(ClaimOutcome::Existing(latest))
}
