use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::completed_workouts::{
    BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout, CompletedWorkoutDetails,
    CompletedWorkoutError, CompletedWorkoutInterval, CompletedWorkoutIntervalGroup,
    CompletedWorkoutMetrics, CompletedWorkoutPowerCurve, CompletedWorkoutRepository,
    CompletedWorkoutSeries, CompletedWorkoutStream, CompletedWorkoutZoneTime,
};

#[derive(Clone)]
pub struct MongoCompletedWorkoutRepository {
    collection: Collection<CompletedWorkoutDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutDocument {
    user_id: String,
    completed_workout_id: String,
    start_date_local: String,
    source_activity_id: Option<String>,
    planned_workout_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    activity_type: Option<String>,
    external_id: Option<String>,
    #[serde(default)]
    trainer: bool,
    duration_seconds: Option<i32>,
    distance_meters: Option<f64>,
    metrics: CompletedWorkoutMetricsDocument,
    details: CompletedWorkoutDetailsDocument,
    details_unavailable_reason: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    power_curve_5s: Option<CompletedWorkoutPowerCurveDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutPowerCurveDocument {
    resolution_seconds: u16,
    sample_period_seconds: u16,
    source_samples: usize,
    valid_power_samples: usize,
    duration_start_seconds: u32,
    duration_step_seconds: u16,
    max_average_watts: Vec<Option<i32>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutMetricsDocument {
    training_stress_score: Option<i32>,
    normalized_power_watts: Option<i32>,
    intensity_factor: Option<f64>,
    efficiency_factor: Option<f64>,
    variability_index: Option<f64>,
    average_power_watts: Option<i32>,
    ftp_watts: Option<i32>,
    total_work_joules: Option<i32>,
    calories: Option<i32>,
    trimp: Option<f64>,
    power_load: Option<i32>,
    heart_rate_load: Option<i32>,
    pace_load: Option<i32>,
    strain_score: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutZoneTimeDocument {
    zone_id: String,
    seconds: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutIntervalDocument {
    id: Option<i32>,
    label: Option<String>,
    interval_type: Option<String>,
    group_id: Option<String>,
    start_index: Option<i32>,
    end_index: Option<i32>,
    start_time_seconds: Option<i32>,
    end_time_seconds: Option<i32>,
    moving_time_seconds: Option<i32>,
    elapsed_time_seconds: Option<i32>,
    distance_meters: Option<f64>,
    average_power_watts: Option<i32>,
    normalized_power_watts: Option<i32>,
    training_stress_score: Option<f64>,
    average_heart_rate_bpm: Option<i32>,
    average_cadence_rpm: Option<f64>,
    average_speed_mps: Option<f64>,
    average_stride_meters: Option<f64>,
    zone: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutIntervalGroupDocument {
    id: String,
    count: Option<i32>,
    start_index: Option<i32>,
    moving_time_seconds: Option<i32>,
    elapsed_time_seconds: Option<i32>,
    distance_meters: Option<f64>,
    average_power_watts: Option<i32>,
    normalized_power_watts: Option<i32>,
    training_stress_score: Option<f64>,
    average_heart_rate_bpm: Option<i32>,
    average_cadence_rpm: Option<f64>,
    average_speed_mps: Option<f64>,
    average_stride_meters: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutStreamDocument {
    stream_type: String,
    name: Option<String>,
    primary_series: Option<serde_json::Value>,
    secondary_series: Option<serde_json::Value>,
    value_type_is_array: bool,
    custom: bool,
    all_null: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletedWorkoutDetailsDocument {
    intervals: Vec<CompletedWorkoutIntervalDocument>,
    interval_groups: Vec<CompletedWorkoutIntervalGroupDocument>,
    streams: Vec<CompletedWorkoutStreamDocument>,
    interval_summary: Vec<String>,
    skyline_chart: Vec<String>,
    power_zone_times: Vec<CompletedWorkoutZoneTimeDocument>,
    heart_rate_zone_times: Vec<i32>,
    pace_zone_times: Vec<i32>,
    gap_zone_times: Vec<i32>,
}

impl MongoCompletedWorkoutRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("completed_workouts"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), CompletedWorkoutError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "completed_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("completed_workouts_user_completed_workout_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "start_date_local": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("completed_workouts_user_start_date_local".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "source_activity_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("completed_workouts_user_source_activity_id".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl CompletedWorkoutRepository for MongoCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "completed_workout_id": &completed_workout_id,
                })
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "source_activity_id": &source_activity_id,
                })
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! { "user_id": &user_id })
                .sort(doc! { "start_date_local": -1, "completed_workout_id": -1 })
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find(doc! { "user_id": &user_id })
                .sort(doc! { "start_date_local": 1, "completed_workout_id": 1 })
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
                .sort(doc! { "start_date_local": 1, "completed_workout_id": 1 })
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
        workout: CompletedWorkout,
    ) -> CompletedWorkoutBoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let document = map_workout_to_document(&workout);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "completed_workout_id": &document.completed_workout_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?;
            Ok(workout)
        })
    }

    fn set_power_curve_5s_if_missing(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        curve: CompletedWorkoutPowerCurve,
    ) -> CompletedWorkoutBoxFuture<Result<(), CompletedWorkoutError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        let curve_doc = map_power_curve_to_document(&curve);
        Box::pin(async move {
            let curve_bson = mongodb::bson::to_document(&curve_doc)
                .map_err(|err| CompletedWorkoutError::Repository(err.to_string()))?;
            collection
                .update_one(
                    doc! {
                        "user_id": &user_id,
                        "completed_workout_id": &completed_workout_id,
                        "$or": [
                            { "power_curve_5s": { "$exists": false } },
                            { "power_curve_5s": null },
                        ],
                    },
                    doc! { "$set": { "power_curve_5s": curve_bson } },
                )
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_workout_to_document(workout: &CompletedWorkout) -> CompletedWorkoutDocument {
    CompletedWorkoutDocument {
        user_id: workout.user_id.clone(),
        completed_workout_id: workout.completed_workout_id.clone(),
        start_date_local: workout.start_date_local.clone(),
        source_activity_id: workout.source_activity_id.clone(),
        planned_workout_id: workout.planned_workout_id.clone(),
        name: workout.name.clone(),
        description: workout.description.clone(),
        activity_type: workout.activity_type.clone(),
        external_id: workout.external_id.clone(),
        trainer: workout.trainer,
        duration_seconds: workout.duration_seconds,
        distance_meters: workout.distance_meters,
        metrics: map_metrics_to_document(&workout.metrics),
        details: map_details_to_document(&workout.details),
        details_unavailable_reason: workout.details_unavailable_reason.clone(),
        power_curve_5s: workout
            .power_curve_5s
            .as_ref()
            .map(map_power_curve_to_document),
    }
}

fn map_document_to_domain(
    document: CompletedWorkoutDocument,
) -> Result<CompletedWorkout, CompletedWorkoutError> {
    let mut workout = CompletedWorkout::new(
        document.completed_workout_id,
        document.user_id,
        document.start_date_local,
        document.source_activity_id,
        document.planned_workout_id,
        document.name,
        document.description,
        document.activity_type,
        document.external_id,
        document.trainer,
        document.duration_seconds,
        document.distance_meters,
        map_metrics_to_domain(document.metrics),
        map_details_to_domain(document.details),
        document.details_unavailable_reason,
    );
    workout.power_curve_5s = document.power_curve_5s.map(map_power_curve_to_domain);
    Ok(workout)
}

fn map_power_curve_to_document(
    curve: &CompletedWorkoutPowerCurve,
) -> CompletedWorkoutPowerCurveDocument {
    CompletedWorkoutPowerCurveDocument {
        resolution_seconds: curve.resolution_seconds,
        sample_period_seconds: curve.sample_period_seconds,
        source_samples: curve.source_samples,
        valid_power_samples: curve.valid_power_samples,
        duration_start_seconds: curve.duration_start_seconds,
        duration_step_seconds: curve.duration_step_seconds,
        max_average_watts: curve.max_average_watts.clone(),
    }
}

fn map_power_curve_to_domain(
    doc: CompletedWorkoutPowerCurveDocument,
) -> CompletedWorkoutPowerCurve {
    CompletedWorkoutPowerCurve {
        resolution_seconds: doc.resolution_seconds,
        sample_period_seconds: doc.sample_period_seconds,
        source_samples: doc.source_samples,
        valid_power_samples: doc.valid_power_samples,
        duration_start_seconds: doc.duration_start_seconds,
        duration_step_seconds: doc.duration_step_seconds,
        max_average_watts: doc.max_average_watts,
    }
}

fn map_metrics_to_document(metrics: &CompletedWorkoutMetrics) -> CompletedWorkoutMetricsDocument {
    CompletedWorkoutMetricsDocument {
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

fn map_metrics_to_domain(metrics: CompletedWorkoutMetricsDocument) -> CompletedWorkoutMetrics {
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

fn map_details_to_document(details: &CompletedWorkoutDetails) -> CompletedWorkoutDetailsDocument {
    CompletedWorkoutDetailsDocument {
        intervals: details
            .intervals
            .iter()
            .map(map_interval_to_document)
            .collect(),
        interval_groups: details
            .interval_groups
            .iter()
            .map(map_interval_group_to_document)
            .collect(),
        streams: details.streams.iter().map(map_stream_to_document).collect(),
        interval_summary: details.interval_summary.clone(),
        skyline_chart: details.skyline_chart.clone(),
        power_zone_times: details
            .power_zone_times
            .iter()
            .map(map_zone_time_to_document)
            .collect(),
        heart_rate_zone_times: details.heart_rate_zone_times.clone(),
        pace_zone_times: details.pace_zone_times.clone(),
        gap_zone_times: details.gap_zone_times.clone(),
    }
}

fn map_details_to_domain(details: CompletedWorkoutDetailsDocument) -> CompletedWorkoutDetails {
    CompletedWorkoutDetails {
        intervals: details
            .intervals
            .into_iter()
            .map(map_interval_to_domain)
            .collect(),
        interval_groups: details
            .interval_groups
            .into_iter()
            .map(map_interval_group_to_domain)
            .collect(),
        streams: details
            .streams
            .into_iter()
            .map(map_stream_to_domain)
            .collect(),
        interval_summary: details.interval_summary,
        skyline_chart: details.skyline_chart,
        power_zone_times: details
            .power_zone_times
            .into_iter()
            .map(map_zone_time_to_domain)
            .collect(),
        heart_rate_zone_times: details.heart_rate_zone_times,
        pace_zone_times: details.pace_zone_times,
        gap_zone_times: details.gap_zone_times,
    }
}

fn map_zone_time_to_document(
    zone_time: &CompletedWorkoutZoneTime,
) -> CompletedWorkoutZoneTimeDocument {
    CompletedWorkoutZoneTimeDocument {
        zone_id: zone_time.zone_id.clone(),
        seconds: zone_time.seconds,
    }
}

fn map_zone_time_to_domain(
    zone_time: CompletedWorkoutZoneTimeDocument,
) -> CompletedWorkoutZoneTime {
    CompletedWorkoutZoneTime {
        zone_id: zone_time.zone_id,
        seconds: zone_time.seconds,
    }
}

fn map_interval_to_document(
    interval: &CompletedWorkoutInterval,
) -> CompletedWorkoutIntervalDocument {
    CompletedWorkoutIntervalDocument {
        id: interval.id,
        label: interval.label.clone(),
        interval_type: interval.interval_type.clone(),
        group_id: interval.group_id.clone(),
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

fn map_interval_to_domain(interval: CompletedWorkoutIntervalDocument) -> CompletedWorkoutInterval {
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

fn map_interval_group_to_document(
    group: &CompletedWorkoutIntervalGroup,
) -> CompletedWorkoutIntervalGroupDocument {
    CompletedWorkoutIntervalGroupDocument {
        id: group.id.clone(),
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

fn map_interval_group_to_domain(
    group: CompletedWorkoutIntervalGroupDocument,
) -> CompletedWorkoutIntervalGroup {
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

fn map_stream_to_document(stream: &CompletedWorkoutStream) -> CompletedWorkoutStreamDocument {
    CompletedWorkoutStreamDocument {
        stream_type: stream.stream_type.clone(),
        name: stream.name.clone(),
        primary_series: map_series_to_value(stream.primary_series.as_ref()),
        secondary_series: map_series_to_value(stream.secondary_series.as_ref()),
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}

fn map_stream_to_domain(stream: CompletedWorkoutStreamDocument) -> CompletedWorkoutStream {
    CompletedWorkoutStream {
        stream_type: stream.stream_type,
        name: stream.name,
        primary_series: map_stream_series(stream.primary_series),
        secondary_series: map_stream_series(stream.secondary_series),
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}

fn map_series_to_value(series: Option<&CompletedWorkoutSeries>) -> Option<serde_json::Value> {
    match series? {
        CompletedWorkoutSeries::Integers(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Floats(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Bools(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Strings(values) => Some(serde_json::json!(values)),
    }
}

fn map_stream_series(value: Option<serde_json::Value>) -> Option<CompletedWorkoutSeries> {
    let value = value?;
    let serde_json::Value::Array(items) = value else {
        return None;
    };

    if items.iter().all(|item| item.as_i64().is_some()) {
        return Some(CompletedWorkoutSeries::Integers(
            items.into_iter().filter_map(|item| item.as_i64()).collect(),
        ));
    }

    if items.iter().all(|item| item.as_f64().is_some()) {
        return Some(CompletedWorkoutSeries::Floats(
            items.into_iter().filter_map(|item| item.as_f64()).collect(),
        ));
    }

    if items.iter().all(|item| item.as_bool().is_some()) {
        return Some(CompletedWorkoutSeries::Bools(
            items
                .into_iter()
                .filter_map(|item| item.as_bool())
                .collect(),
        ));
    }

    if items.iter().all(|item| item.as_str().is_some()) {
        return Some(CompletedWorkoutSeries::Strings(
            items
                .into_iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::domain::completed_workouts::{
        compute_power_curve, CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutInterval,
        CompletedWorkoutIntervalGroup, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    };

    use super::{map_document_to_domain, map_workout_to_document};

    #[test]
    fn completed_workout_document_round_trip_preserves_fields() {
        let workout = CompletedWorkout::new(
            "completed-1".to_string(),
            "user-1".to_string(),
            "2026-05-10T08:00:00".to_string(),
            Some("activity-1".to_string()),
            Some("planned-1".to_string()),
            Some("Threshold Ride".to_string()),
            Some("Strong day".to_string()),
            Some("Ride".to_string()),
            Some("external-1".to_string()),
            true,
            Some(3600),
            Some(42_000.0),
            CompletedWorkoutMetrics {
                training_stress_score: Some(88),
                normalized_power_watts: Some(250),
                intensity_factor: Some(0.86),
                efficiency_factor: Some(1.1),
                variability_index: Some(1.03),
                average_power_watts: Some(225),
                ftp_watts: Some(290),
                total_work_joules: Some(800),
                calories: Some(950),
                trimp: Some(100.0),
                power_load: Some(50),
                heart_rate_load: Some(25),
                pace_load: Some(10),
                strain_score: Some(12.5),
            },
            CompletedWorkoutDetails {
                intervals: vec![CompletedWorkoutInterval {
                    id: Some(1),
                    label: Some("Main set".to_string()),
                    interval_type: Some("steady".to_string()),
                    group_id: Some("group-1".to_string()),
                    start_index: Some(10),
                    end_index: Some(20),
                    start_time_seconds: Some(600),
                    end_time_seconds: Some(1200),
                    moving_time_seconds: Some(600),
                    elapsed_time_seconds: Some(620),
                    distance_meters: Some(5000.0),
                    average_power_watts: Some(260),
                    normalized_power_watts: Some(268),
                    training_stress_score: Some(22.5),
                    average_heart_rate_bpm: Some(165),
                    average_cadence_rpm: Some(92.0),
                    average_speed_mps: Some(10.2),
                    average_stride_meters: None,
                    zone: Some(4),
                }],
                interval_groups: vec![CompletedWorkoutIntervalGroup {
                    id: "group-1".to_string(),
                    count: Some(3),
                    start_index: Some(10),
                    moving_time_seconds: Some(1800),
                    elapsed_time_seconds: Some(1860),
                    distance_meters: Some(15000.0),
                    average_power_watts: Some(250),
                    normalized_power_watts: Some(260),
                    training_stress_score: Some(60.0),
                    average_heart_rate_bpm: Some(160),
                    average_cadence_rpm: Some(90.0),
                    average_speed_mps: Some(9.8),
                    average_stride_meters: None,
                }],
                streams: vec![CompletedWorkoutStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 220, 260])),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                }],
                interval_summary: vec!["threshold".to_string()],
                skyline_chart: vec!["abc".to_string()],
                power_zone_times: vec![CompletedWorkoutZoneTime {
                    zone_id: "z4".to_string(),
                    seconds: 1200,
                }],
                heart_rate_zone_times: vec![300, 900],
                pace_zone_times: vec![10],
                gap_zone_times: vec![20],
            },
            Some("details unavailable".to_string()),
        );

        let mapped = map_document_to_domain(map_workout_to_document(&workout)).unwrap();

        assert_eq!(mapped, workout);
    }

    #[test]
    fn power_curve_round_trip_preserves_5s_curve() {
        let mut workout = CompletedWorkout::new(
            "completed-1".to_string(),
            "user-1".to_string(),
            "2026-05-10T08:00:00".to_string(),
            Some("activity-1".to_string()),
            Some("planned-1".to_string()),
            Some("Threshold Ride".to_string()),
            Some("Strong day".to_string()),
            Some("Ride".to_string()),
            Some("external-1".to_string()),
            true,
            Some(3600),
            Some(42_000.0),
            CompletedWorkoutMetrics::default(),
            CompletedWorkoutDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: vec![CompletedWorkoutStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    primary_series: Some(CompletedWorkoutSeries::Integers(vec![
                        200, 250, 300, 275, 225, 180, 240, 260, 255, 215,
                    ])),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                }],
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            None,
        );
        workout.power_curve_5s = compute_power_curve(&workout, 5).ok();

        let mapped = map_document_to_domain(map_workout_to_document(&workout)).unwrap();
        assert_eq!(mapped, workout);
        assert!(mapped.power_curve_5s.is_some());
    }

    #[test]
    fn legacy_document_without_power_curve_maps_to_none() {
        let doc = mongodb::bson::doc! {
            "user_id": "user-1",
            "completed_workout_id": "completed-1",
            "start_date_local": "2026-05-10T08:00:00",
            "source_activity_id": mongodb::bson::Bson::Null,
            "planned_workout_id": mongodb::bson::Bson::Null,
            "name": "Threshold Ride",
            "description": mongodb::bson::Bson::Null,
            "activity_type": "Ride",
            "external_id": mongodb::bson::Bson::Null,
            "trainer": true,
            "duration_seconds": 3600i32,
            "distance_meters": mongodb::bson::Bson::Null,
            "metrics": {
                "training_stress_score": mongodb::bson::Bson::Null,
                "normalized_power_watts": mongodb::bson::Bson::Null,
                "intensity_factor": mongodb::bson::Bson::Null,
                "efficiency_factor": mongodb::bson::Bson::Null,
                "variability_index": mongodb::bson::Bson::Null,
                "average_power_watts": mongodb::bson::Bson::Null,
                "ftp_watts": mongodb::bson::Bson::Null,
                "total_work_joules": mongodb::bson::Bson::Null,
                "calories": mongodb::bson::Bson::Null,
                "trimp": mongodb::bson::Bson::Null,
                "power_load": mongodb::bson::Bson::Null,
                "heart_rate_load": mongodb::bson::Bson::Null,
                "pace_load": mongodb::bson::Bson::Null,
                "strain_score": mongodb::bson::Bson::Null,
            },
            "details": {
                "intervals": [],
                "interval_groups": [],
                "streams": [],
                "interval_summary": [],
                "skyline_chart": [],
                "power_zone_times": [],
                "heart_rate_zone_times": [],
                "pace_zone_times": [],
                "gap_zone_times": [],
            },
            "details_unavailable_reason": mongodb::bson::Bson::Null,
        };
        let document: super::CompletedWorkoutDocument =
            mongodb::bson::from_document(doc).expect("legacy document should deserialize");
        assert!(document.power_curve_5s.is_none());

        let workout = map_document_to_domain(document).unwrap();
        assert!(workout.power_curve_5s.is_none());
    }
}
