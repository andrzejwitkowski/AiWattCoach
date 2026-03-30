use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::intervals::{
    Activity, ActivityDetails, ActivityFallbackIdentity, ActivityMetrics, ActivityRepositoryPort,
    DateRange, IntervalsError,
};

use crate::common::BoxFuture;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RepoCall {
    Upsert(String),
    UpsertMany(usize),
    FindRange {
        user_id: String,
        oldest: String,
        newest: String,
    },
    FindExternalId(String),
    FindFallbackIdentity(String),
}

#[derive(Clone, Default)]
pub(crate) struct FakeActivityRepository {
    pub(crate) stored: Arc<Mutex<HashMap<String, Vec<Activity>>>>,
    pub(crate) call_log: Arc<Mutex<Vec<RepoCall>>>,
    delete_error: Option<IntervalsError>,
    sequence: Option<Arc<Mutex<Vec<String>>>>,
    upsert_error: Option<IntervalsError>,
}

impl FakeActivityRepository {
    pub(crate) fn with_sequence(sequence: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            sequence: Some(sequence),
            ..Self::default()
        }
    }

    pub(crate) fn with_existing(user_id: &str, activity: Activity) -> Self {
        let mut stored = HashMap::new();
        stored.insert(user_id.to_string(), vec![activity]);
        Self {
            stored: Arc::new(Mutex::new(stored)),
            ..Self::default()
        }
    }

    pub(crate) fn with_upsert_error(error: IntervalsError) -> Self {
        Self {
            upsert_error: Some(error),
            ..Self::default()
        }
    }

    pub(crate) fn with_sequence_and_delete_error(
        sequence: Arc<Mutex<Vec<String>>>,
        error: IntervalsError,
    ) -> Self {
        Self {
            delete_error: Some(error),
            sequence: Some(sequence),
            ..Self::default()
        }
    }
}

fn merge_activity_for_storage(existing: Option<Activity>, incoming: Activity) -> Activity {
    let Some(existing) = existing else {
        return incoming;
    };

    if activity_detail_richness(&incoming) >= activity_detail_richness(&existing) {
        return incoming;
    }

    Activity {
        id: incoming.id,
        athlete_id: incoming.athlete_id.or(existing.athlete_id),
        start_date_local: incoming.start_date_local,
        start_date: incoming.start_date.or(existing.start_date),
        name: incoming.name.or(existing.name),
        description: incoming.description.or(existing.description),
        activity_type: incoming.activity_type.or(existing.activity_type),
        source: incoming.source.or(existing.source),
        external_id: incoming.external_id.or(existing.external_id),
        device_name: incoming.device_name.or(existing.device_name),
        distance_meters: incoming.distance_meters.or(existing.distance_meters),
        moving_time_seconds: incoming
            .moving_time_seconds
            .or(existing.moving_time_seconds),
        elapsed_time_seconds: incoming
            .elapsed_time_seconds
            .or(existing.elapsed_time_seconds),
        total_elevation_gain_meters: incoming
            .total_elevation_gain_meters
            .or(existing.total_elevation_gain_meters),
        total_elevation_loss_meters: incoming
            .total_elevation_loss_meters
            .or(existing.total_elevation_loss_meters),
        average_speed_mps: incoming.average_speed_mps.or(existing.average_speed_mps),
        max_speed_mps: incoming.max_speed_mps.or(existing.max_speed_mps),
        average_heart_rate_bpm: incoming
            .average_heart_rate_bpm
            .or(existing.average_heart_rate_bpm),
        max_heart_rate_bpm: incoming.max_heart_rate_bpm.or(existing.max_heart_rate_bpm),
        average_cadence_rpm: incoming
            .average_cadence_rpm
            .or(existing.average_cadence_rpm),
        trainer: incoming.trainer || existing.trainer,
        commute: incoming.commute || existing.commute,
        race: incoming.race || existing.race,
        has_heart_rate: incoming.has_heart_rate || existing.has_heart_rate,
        stream_types: prefer_non_empty(incoming.stream_types, existing.stream_types),
        tags: prefer_non_empty(incoming.tags, existing.tags),
        metrics: merge_activity_metrics(existing.metrics, incoming.metrics),
        details: merge_activity_details(existing.details, incoming.details),
        details_unavailable_reason: incoming
            .details_unavailable_reason
            .or(existing.details_unavailable_reason),
    }
}

fn activity_detail_richness(activity: &Activity) -> usize {
    usize::from(!activity.details.intervals.is_empty())
        + usize::from(!activity.details.interval_groups.is_empty())
        + usize::from(!activity.details.streams.is_empty())
        + usize::from(!activity.details.interval_summary.is_empty())
        + usize::from(!activity.details.skyline_chart.is_empty())
        + usize::from(!activity.details.power_zone_times.is_empty())
        + usize::from(!activity.details.heart_rate_zone_times.is_empty())
        + usize::from(!activity.details.pace_zone_times.is_empty())
        + usize::from(!activity.details.gap_zone_times.is_empty())
}

fn merge_activity_metrics(existing: ActivityMetrics, incoming: ActivityMetrics) -> ActivityMetrics {
    ActivityMetrics {
        training_stress_score: incoming
            .training_stress_score
            .or(existing.training_stress_score),
        normalized_power_watts: incoming
            .normalized_power_watts
            .or(existing.normalized_power_watts),
        intensity_factor: incoming.intensity_factor.or(existing.intensity_factor),
        efficiency_factor: incoming.efficiency_factor.or(existing.efficiency_factor),
        variability_index: incoming.variability_index.or(existing.variability_index),
        average_power_watts: incoming
            .average_power_watts
            .or(existing.average_power_watts),
        ftp_watts: incoming.ftp_watts.or(existing.ftp_watts),
        total_work_joules: incoming.total_work_joules.or(existing.total_work_joules),
        calories: incoming.calories.or(existing.calories),
        trimp: incoming.trimp.or(existing.trimp),
        power_load: incoming.power_load.or(existing.power_load),
        heart_rate_load: incoming.heart_rate_load.or(existing.heart_rate_load),
        pace_load: incoming.pace_load.or(existing.pace_load),
        strain_score: incoming.strain_score.or(existing.strain_score),
    }
}

fn merge_activity_details(existing: ActivityDetails, incoming: ActivityDetails) -> ActivityDetails {
    ActivityDetails {
        intervals: prefer_non_empty(incoming.intervals, existing.intervals),
        interval_groups: prefer_non_empty(incoming.interval_groups, existing.interval_groups),
        streams: prefer_non_empty(incoming.streams, existing.streams),
        interval_summary: prefer_non_empty(incoming.interval_summary, existing.interval_summary),
        skyline_chart: prefer_non_empty(incoming.skyline_chart, existing.skyline_chart),
        power_zone_times: prefer_non_empty(incoming.power_zone_times, existing.power_zone_times),
        heart_rate_zone_times: prefer_non_empty(
            incoming.heart_rate_zone_times,
            existing.heart_rate_zone_times,
        ),
        pace_zone_times: prefer_non_empty(incoming.pace_zone_times, existing.pace_zone_times),
        gap_zone_times: prefer_non_empty(incoming.gap_zone_times, existing.gap_zone_times),
    }
}

fn prefer_non_empty<T>(incoming: Vec<T>, existing: Vec<T>) -> Vec<T> {
    if incoming.is_empty() {
        existing
    } else {
        incoming
    }
}

fn activity_date(start_date_local: &str) -> &str {
    start_date_local.get(..10).unwrap_or(start_date_local)
}

impl ActivityRepositoryPort for FakeActivityRepository {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let error = self.upsert_error.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::Upsert(activity.id.clone()));
            if let Some(error) = error {
                return Err(error);
            }
            let mut store = store.lock().unwrap();
            let activities = store.entry(user_id).or_default();
            let existing = activities
                .iter()
                .find(|current| current.id == activity.id)
                .cloned();
            let activity = merge_activity_for_storage(existing, activity);
            activities.retain(|existing| existing.id != activity.id);
            activities.push(activity.clone());
            Ok(activity)
        })
    }

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::UpsertMany(activities.len()));
            let mut store = store.lock().unwrap();
            let existing = store.entry(user_id).or_default();
            let mut stored = Vec::with_capacity(activities.len());
            for activity in activities {
                let current = existing
                    .iter()
                    .find(|candidate| candidate.id == activity.id)
                    .cloned();
                let merged = merge_activity_for_storage(current, activity);
                existing.retain(|current| current.id != merged.id);
                existing.push(merged.clone());
                stored.push(merged);
            }
            Ok(stored)
        })
    }

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            let repo_user_id = user_id.clone();
            calls.lock().unwrap().push(RepoCall::FindRange {
                user_id,
                oldest: oldest.clone(),
                newest: newest.clone(),
            });
            let activities = store
                .lock()
                .unwrap()
                .get(&repo_user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities
                .into_iter()
                .filter(|activity| activity_date(&activity.start_date_local) >= oldest.as_str())
                .filter(|activity| activity_date(&activity.start_date_local) <= newest.as_str())
                .collect())
        })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let activities = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities
                .into_iter()
                .find(|activity| activity.id == activity_id))
        })
    }

    fn find_by_user_id_and_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::FindExternalId(external_id.clone()));
            let activities = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities.into_iter().find(|activity| {
                activity
                    .external_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    == Some(external_id.as_str())
            }))
        })
    }

    fn find_by_user_id_and_fallback_identity(
        &self,
        user_id: &str,
        identity: &str,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        let identity = identity.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::FindFallbackIdentity(identity.clone()));
            let activities = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities
                .into_iter()
                .filter(|activity| {
                    ActivityFallbackIdentity::from_activity(activity)
                        .map(|candidate| candidate.as_fingerprint())
                        == Some(identity.clone())
                })
                .collect())
        })
    }

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        let store = self.stored.clone();
        let sequence = self.sequence.clone();
        let error = self.delete_error.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(sequence) = sequence {
                sequence
                    .lock()
                    .unwrap()
                    .push(format!("repo_delete:{activity_id}"));
            }
            if let Some(error) = error {
                return Err(error);
            }
            if let Some(activities) = store.lock().unwrap().get_mut(&user_id) {
                activities.retain(|activity| activity.id != activity_id);
            }
            Ok(())
        })
    }
}
