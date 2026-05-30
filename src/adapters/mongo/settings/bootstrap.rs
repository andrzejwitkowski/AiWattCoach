use futures::TryStreamExt;
use mongodb::{bson::doc, Collection};

use super::documents::SettingsDocument;
use crate::adapters::mongo::time::optional_epoch_seconds_to_bson_datetime;
use crate::domain::settings::{SettingsError, WahooConfig, WahooUserIdBackfillCandidate};

#[derive(Clone, Debug, Deserialize)]
pub(super) struct WahooPollBootstrapUserDocument {
    pub(super) user_id: String,
    pub(super) wahoo: Option<WahooPollBootstrapWahooDocument>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct WahooPollBootstrapWahooDocument {
    pub(super) access_token: Option<String>,
    pub(super) refresh_token: Option<String>,
    pub(super) user_id: Option<i64>,
    pub(super) connected: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntervalsPollBootstrapUser {
    pub user_id: String,
    pub desired_active: bool,
    pub intervals_updated_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct IntervalsPollBootstrapUserDocument {
    pub(super) user_id: String,
    pub(super) updated_at_epoch_seconds: Option<i64>,
    pub(super) intervals: Option<IntervalsPollBootstrapIntervalsDocument>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct IntervalsPollBootstrapIntervalsDocument {
    pub(super) api_key: Option<String>,
    pub(super) athlete_id: Option<String>,
    pub(super) connected: Option<bool>,
    pub(super) updated_at_epoch_seconds: Option<i64>,
}

use serde::Deserialize;

pub(super) async fn list_intervals_poll_bootstrap_users_impl(
    collection: &Collection<SettingsDocument>,
    user_ids: &[String],
) -> Result<Vec<IntervalsPollBootstrapUser>, SettingsError> {
    let poll_user_ids = user_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let collection = collection.clone_with_type::<IntervalsPollBootstrapUserDocument>();
    let filter = build_intervals_poll_bootstrap_filter(user_ids);
    let documents = collection
        .find(filter)
        .projection(doc! {
            "_id": 0,
            "user_id": 1,
            "intervals": 1,
            "updated_at_epoch_seconds": 1,
        })
        .sort(doc! { "user_id": 1 })
        .await
        .map_err(|error| SettingsError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| SettingsError::Repository(error.to_string()))?;

    Ok(documents
        .into_iter()
        .filter(|document| {
            if poll_user_ids.contains(&document.user_id) {
                return true;
            }

            should_include_non_requested_bootstrap_user(document)
        })
        .map(|document| {
            let desired_active = document
                .intervals
                .as_ref()
                .is_some_and(is_bootstrap_active_intervals);
            let intervals_updated_at_epoch_seconds = bootstrap_intervals_updated_at(&document);

            IntervalsPollBootstrapUser {
                user_id: document.user_id,
                desired_active,
                intervals_updated_at_epoch_seconds,
            }
        })
        .collect())
}

pub(super) async fn list_wahoo_user_id_backfill_candidates_impl(
    collection: &Collection<SettingsDocument>,
) -> Result<Vec<WahooUserIdBackfillCandidate>, SettingsError> {
    let collection = collection.clone_with_type::<WahooPollBootstrapUserDocument>();
    let documents = collection
        .find(doc! {
            "$and": [
                { "wahoo.refresh_token": { "$type": "string", "$regex": "\\S" } },
                { "wahoo.connected": { "$ne": false } },
                { "wahoo.user_id": null },
            ]
        })
        .projection(doc! {
            "_id": 0,
            "user_id": 1,
            "wahoo": 1,
        })
        .sort(doc! { "user_id": 1 })
        .await
        .map_err(|error| SettingsError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| SettingsError::Repository(error.to_string()))?;

    Ok(documents
        .into_iter()
        .filter_map(|document| {
            let wahoo = document.wahoo?;
            Some(WahooUserIdBackfillCandidate {
                user_id: document.user_id,
                wahoo: WahooConfig {
                    access_token: wahoo.access_token,
                    refresh_token: wahoo.refresh_token,
                    expires_at_epoch_seconds: None,
                    user_id: wahoo.user_id,
                    connected: wahoo.connected.unwrap_or(true),
                    updated_at_epoch_seconds: None,
                },
            })
        })
        .collect())
}

pub(super) async fn backfill_wahoo_user_id_impl(
    collection: &Collection<SettingsDocument>,
    user_id: &str,
    wahoo_user_id: i64,
    updated_at_epoch_seconds: i64,
) -> Result<(), SettingsError> {
    let result = collection
        .update_one(
            doc! {
                "user_id": user_id,
                "$or": [
                    { "wahoo.user_id": null },
                    { "wahoo.user_id": wahoo_user_id },
                ]
            },
            doc! {
                "$set": {
                    "wahoo.user_id": wahoo_user_id,
                    "wahoo.updated_at_epoch_seconds": updated_at_epoch_seconds,
                    "wahoo.updated_at": optional_epoch_seconds_to_bson_datetime(
                        Some(updated_at_epoch_seconds),
                        "wahoo.updated_at"
                    ).map_err(SettingsError::Repository)?,
                }
            },
        )
        .await
        .map_err(|error| SettingsError::Repository(error.to_string()))?;
    if result.matched_count == 0 {
        return Err(SettingsError::Repository(format!(
            "no settings document updated for user_id={user_id} wahoo_user_id={wahoo_user_id}",
        )));
    }
    Ok(())
}

pub(super) fn has_non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn is_bootstrap_active_intervals(
    intervals: &IntervalsPollBootstrapIntervalsDocument,
) -> bool {
    has_non_empty(intervals.api_key.as_deref())
        && has_non_empty(intervals.athlete_id.as_deref())
        && intervals.connected != Some(false)
}

fn should_include_non_requested_bootstrap_user(
    document: &IntervalsPollBootstrapUserDocument,
) -> bool {
    document
        .intervals
        .as_ref()
        .is_some_and(is_bootstrap_active_intervals)
}

pub(super) fn bootstrap_intervals_updated_at(
    document: &IntervalsPollBootstrapUserDocument,
) -> Option<i64> {
    document
        .intervals
        .as_ref()
        .and_then(|intervals| intervals.updated_at_epoch_seconds)
        .or(document.updated_at_epoch_seconds)
}

pub(super) fn build_intervals_poll_bootstrap_filter(
    user_ids: &[String],
) -> mongodb::bson::Document {
    let mut filter_clauses = vec![doc! {
        "$and": [
            { "intervals.api_key": { "$type": "string", "$regex": "\\S" } },
            { "intervals.athlete_id": { "$type": "string", "$regex": "\\S" } },
            { "intervals.connected": { "$ne": false } },
        ]
    }];

    if !user_ids.is_empty() {
        filter_clauses.push(doc! { "user_id": { "$in": user_ids } });
    }

    doc! { "$or": filter_clauses }
}
