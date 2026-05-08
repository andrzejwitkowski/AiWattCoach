use mongodb::bson::Document;
use mongodb::Collection;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::error::is_duplicate_key_error;

pub(super) enum ClaimOutcome<Op> {
    Claimed(Op),
    Existing(Op),
}

pub(super) struct OpMetadata {
    pub attempt_count: i64,
    pub updated_at_epoch_seconds: i64,
    pub last_attempt_at_epoch_seconds: i64,
}

pub(super) struct ClaimInput<Doc, Op>
where
    Doc: Send + Sync,
{
    pub collection: Collection<Doc>,
    pub document: Doc,
    pub operation: Op,
    pub stale_before_epoch_seconds: i64,
}

pub(super) async fn mongo_claim_pending<Doc, Op>(
    input: ClaimInput<Doc, Op>,
    unique_filter: impl Fn() -> Document,
    map_document_to_operation: impl Fn(Doc) -> Result<Op, String>,
    is_reclaimable: impl Fn(&Op, i64) -> bool,
    op_metadata: impl Fn(&Op) -> OpMetadata,
    build_reclaimed: impl FnOnce(&Op, &Op, i64) -> Result<(Op, Doc), String>,
) -> Result<ClaimOutcome<Op>, String>
where
    Doc: Serialize + DeserializeOwned + Unpin + Send + Sync,
{
    let inserted = input
        .collection
        .insert_one(&input.document)
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
        return Ok(ClaimOutcome::Claimed(input.operation));
    }

    let existing_document = input
        .collection
        .find_one(unique_filter())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "claimed operation disappeared before reload".to_string())?;

    let existing = map_document_to_operation(existing_document)?;

    if !is_reclaimable(&existing, input.stale_before_epoch_seconds) {
        return Ok(ClaimOutcome::Existing(existing));
    }

    let pending_meta = op_metadata(&input.operation);
    let (reclaimed_op, reclaimed_document) = build_reclaimed(
        &existing,
        &input.operation,
        pending_meta.last_attempt_at_epoch_seconds,
    )?;

    let existing_meta = op_metadata(&existing);
    let mut cas_filter = unique_filter();
    cas_filter.insert("attempt_count", existing_meta.attempt_count);
    cas_filter.insert(
        "updated_at_epoch_seconds",
        existing_meta.updated_at_epoch_seconds,
    );

    let replaced = input
        .collection
        .find_one_and_replace(cas_filter, &reclaimed_document)
        .await
        .map_err(|error| error.to_string())?;

    if replaced.is_some() {
        return Ok(ClaimOutcome::Claimed(reclaimed_op));
    }

    let latest_document = input
        .collection
        .find_one(unique_filter())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "reclaimed operation disappeared before reload".to_string())?;

    let latest = map_document_to_operation(latest_document)?;
    Ok(ClaimOutcome::Existing(latest))
}
