use futures::TryStreamExt;
use mongodb::{bson::doc, Collection};
use serde::Deserialize;

use crate::domain::{
    completed_workouts::{
        BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout, CompletedWorkoutDetails,
        CompletedWorkoutError, CompletedWorkoutInterval, CompletedWorkoutIntervalGroup,
        CompletedWorkoutMetrics, CompletedWorkoutRepository, CompletedWorkoutStream,
        CompletedWorkoutZoneTime,
    },
    intervals::{
        Activity, ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
        ActivityStream, ActivityZoneTime,
    },
};

#[derive(Clone)]
pub struct MongoCompletedWorkoutRepository {
    collection: Collection<ActivityDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct ActivityDocument {
    user_id: String,
    activity_id: String,
    payload: Activity,
}

impl MongoCompletedWorkoutRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("intervals_activities"),
        }
    }
}

impl CompletedWorkoutRepository for MongoCompletedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find(doc! { "user_id": &user_id })
                .sort(doc! { "start_date_local": 1, "activity_id": 1 })
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .into_iter()
                .map(map_document_to_domain)
                .collect()
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "start_date_local": {
                        "$gte": format!("{oldest}T00:00:00"),
                        "$lte": format!("{newest}T23:59:59"),
                    },
                })
                .sort(doc! { "start_date_local": 1, "activity_id": 1 })
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .into_iter()
                .map(map_document_to_domain)
                .collect()
        })
    }

    fn upsert(
        &self,
        _workout: CompletedWorkout,
    ) -> CompletedWorkoutBoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        Box::pin(async {
            Err(CompletedWorkoutError::Repository(
                "completed workout bridge repository is read-only; persist through activity storage"
                    .to_string(),
            ))
        })
    }
}

fn map_document_to_domain(
    document: ActivityDocument,
) -> Result<CompletedWorkout, CompletedWorkoutError> {
    Ok(CompletedWorkout::new(
        document.activity_id,
        document.user_id,
        document.payload.start_date_local,
        map_metrics(document.payload.metrics),
        map_details(document.payload.details),
    ))
}

fn map_metrics(metrics: ActivityMetrics) -> CompletedWorkoutMetrics {
    CompletedWorkoutMetrics {
        training_stress_score: metrics.training_stress_score,
        normalized_power_watts: metrics.normalized_power_watts,
        intensity_factor: metrics.intensity_factor,
        efficiency_factor: metrics.efficiency_factor,
        variability_index: metrics.variability_index,
        average_power_watts: metrics.average_power_watts,
        ftp_watts: metrics.ftp_watts,
        total_work_joules: metrics.total_work_joules,
        calories: metrics.calories,
        trimp: metrics.trimp,
        power_load: metrics.power_load,
        heart_rate_load: metrics.heart_rate_load,
        pace_load: metrics.pace_load,
        strain_score: metrics.strain_score,
    }
}

fn map_details(details: ActivityDetails) -> CompletedWorkoutDetails {
    CompletedWorkoutDetails {
        intervals: details.intervals.into_iter().map(map_interval).collect(),
        interval_groups: details
            .interval_groups
            .into_iter()
            .map(map_interval_group)
            .collect(),
        streams: details.streams.into_iter().map(map_stream).collect(),
        interval_summary: details.interval_summary,
        skyline_chart: details.skyline_chart,
        power_zone_times: details
            .power_zone_times
            .into_iter()
            .map(map_zone_time)
            .collect(),
        heart_rate_zone_times: details.heart_rate_zone_times,
        pace_zone_times: details.pace_zone_times,
        gap_zone_times: details.gap_zone_times,
    }
}

fn map_zone_time(zone_time: ActivityZoneTime) -> CompletedWorkoutZoneTime {
    CompletedWorkoutZoneTime {
        zone_id: zone_time.zone_id,
        seconds: zone_time.seconds,
    }
}

fn map_interval(interval: ActivityInterval) -> CompletedWorkoutInterval {
    CompletedWorkoutInterval {
        id: interval.id,
        label: interval.label,
        interval_type: interval.interval_type,
        group_id: interval.group_id,
        start_index: interval.start_index,
        end_index: interval.end_index,
        start_time_seconds: interval.start_time_seconds,
        end_time_seconds: interval.end_time_seconds,
        moving_time_seconds: interval.moving_time_seconds,
        elapsed_time_seconds: interval.elapsed_time_seconds,
        distance_meters: interval.distance_meters,
        average_power_watts: interval.average_power_watts,
        normalized_power_watts: interval.normalized_power_watts,
        training_stress_score: interval.training_stress_score,
        average_heart_rate_bpm: interval.average_heart_rate_bpm,
        average_cadence_rpm: interval.average_cadence_rpm,
        average_speed_mps: interval.average_speed_mps,
        average_stride_meters: interval.average_stride_meters,
        zone: interval.zone,
    }
}

fn map_interval_group(group: ActivityIntervalGroup) -> CompletedWorkoutIntervalGroup {
    CompletedWorkoutIntervalGroup {
        id: group.id,
        count: group.count,
        start_index: group.start_index,
        moving_time_seconds: group.moving_time_seconds,
        elapsed_time_seconds: group.elapsed_time_seconds,
        distance_meters: group.distance_meters,
        average_power_watts: group.average_power_watts,
        normalized_power_watts: group.normalized_power_watts,
        training_stress_score: group.training_stress_score,
        average_heart_rate_bpm: group.average_heart_rate_bpm,
        average_cadence_rpm: group.average_cadence_rpm,
        average_speed_mps: group.average_speed_mps,
        average_stride_meters: group.average_stride_meters,
    }
}

fn map_stream(stream: ActivityStream) -> CompletedWorkoutStream {
    CompletedWorkoutStream {
        stream_type: stream.stream_type,
        name: stream.name,
        primary_series: stream.data,
        secondary_series: stream.data2,
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}
