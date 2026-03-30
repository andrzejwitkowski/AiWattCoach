use crate::domain::intervals::{
    Activity, ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
    ActivityStream, ActivityZoneTime, Event, EventCategory,
};

use super::super::dto::{
    ActivityIntervalGroupResponse, ActivityIntervalResponse, ActivityResponse,
    ActivityStreamResponse, EventResponse,
};

pub(super) fn map_event_response(response: EventResponse) -> Event {
    Event {
        id: response.id,
        start_date_local: response.start_date_local,
        name: response.name,
        category: EventCategory::from_api_str(&response.category),
        description: response.description,
        indoor: response.indoor.unwrap_or(false),
        color: response.color,
        workout_doc: response.workout_doc,
    }
}

pub(super) fn map_activity_response(response: ActivityResponse) -> Activity {
    Activity {
        id: response.id,
        athlete_id: response.icu_athlete_id,
        start_date_local: response.start_date_local,
        start_date: response.start_date,
        name: response.name,
        description: response.description,
        activity_type: response.activity_type,
        source: response.source,
        external_id: response.external_id,
        device_name: response.device_name,
        distance_meters: response.distance,
        moving_time_seconds: response.moving_time,
        elapsed_time_seconds: response.elapsed_time,
        total_elevation_gain_meters: response.total_elevation_gain,
        total_elevation_loss_meters: response.total_elevation_loss,
        average_speed_mps: response.average_speed,
        max_speed_mps: response.max_speed,
        average_heart_rate_bpm: response.average_heartrate,
        max_heart_rate_bpm: response.max_heartrate,
        average_cadence_rpm: response.average_cadence,
        trainer: response.trainer.unwrap_or(false),
        commute: response.commute.unwrap_or(false),
        race: response.race.unwrap_or(false),
        has_heart_rate: response.has_heartrate.unwrap_or(false),
        stream_types: response
            .stream_types
            .unwrap_or_default()
            .into_iter()
            .filter(|stream_type| should_persist_stream_type(stream_type))
            .collect(),
        tags: response.tags.unwrap_or_default(),
        metrics: ActivityMetrics {
            training_stress_score: response.icu_training_load,
            normalized_power_watts: response.icu_weighted_avg_watts,
            intensity_factor: response.icu_intensity,
            efficiency_factor: response.icu_efficiency_factor,
            variability_index: response.icu_variability_index,
            average_power_watts: response.icu_average_watts,
            ftp_watts: response.icu_ftp,
            total_work_joules: response.icu_joules,
            calories: response.calories,
            trimp: response.trimp,
            power_load: response.power_load,
            heart_rate_load: response.hr_load,
            pace_load: response.pace_load,
            strain_score: response.strain_score,
        },
        details: ActivityDetails {
            intervals: response
                .icu_intervals
                .unwrap_or_default()
                .into_iter()
                .map(map_activity_interval)
                .collect(),
            interval_groups: response
                .icu_groups
                .unwrap_or_default()
                .into_iter()
                .map(map_activity_interval_group)
                .collect(),
            streams: Vec::new(),
            interval_summary: response.interval_summary.unwrap_or_default(),
            skyline_chart: response.skyline_chart_bytes.unwrap_or_default(),
            power_zone_times: response
                .icu_zone_times
                .unwrap_or_default()
                .into_iter()
                .map(|zone| ActivityZoneTime {
                    zone_id: zone.id.into_string(),
                    seconds: zone.secs,
                })
                .collect(),
            heart_rate_zone_times: response.icu_hr_zone_times.unwrap_or_default(),
            pace_zone_times: response.pace_zone_times.unwrap_or_default(),
            gap_zone_times: response.gap_zone_times.unwrap_or_default(),
        },
        details_unavailable_reason: None,
    }
}

pub(super) fn map_activity_interval(response: ActivityIntervalResponse) -> ActivityInterval {
    ActivityInterval {
        id: response.id,
        label: response.label,
        interval_type: response.interval_type,
        group_id: response.group_id,
        start_index: response.start_index,
        end_index: response.end_index,
        start_time_seconds: response.start_time,
        end_time_seconds: response.end_time,
        moving_time_seconds: response.moving_time,
        elapsed_time_seconds: response.elapsed_time,
        distance_meters: response.distance,
        average_power_watts: response.average_watts,
        normalized_power_watts: response.weighted_average_watts,
        training_stress_score: response.training_load,
        average_heart_rate_bpm: response.average_heartrate,
        average_cadence_rpm: response.average_cadence,
        average_speed_mps: response.average_speed,
        average_stride_meters: response.average_stride,
        zone: response.zone,
    }
}

pub(super) fn map_activity_interval_group(
    response: ActivityIntervalGroupResponse,
) -> ActivityIntervalGroup {
    ActivityIntervalGroup {
        id: response.id,
        count: response.count,
        start_index: response.start_index,
        moving_time_seconds: response.moving_time,
        elapsed_time_seconds: response.elapsed_time,
        distance_meters: response.distance,
        average_power_watts: response.average_watts,
        normalized_power_watts: response.weighted_average_watts,
        training_stress_score: response.training_load,
        average_heart_rate_bpm: response.average_heartrate,
        average_cadence_rpm: response.average_cadence,
        average_speed_mps: response.average_speed,
        average_stride_meters: response.average_stride,
    }
}

pub(super) fn map_activity_stream(response: ActivityStreamResponse) -> ActivityStream {
    ActivityStream {
        stream_type: response.stream_type,
        name: response.name,
        data: response.data,
        data2: response.data2,
        value_type_is_array: response.value_type_is_array,
        custom: response.custom,
        all_null: response.all_null,
    }
}

pub(super) fn should_persist_stream(stream: &ActivityStreamResponse) -> bool {
    should_persist_stream_type(&stream.stream_type)
}

fn should_persist_stream_type(stream_type: &str) -> bool {
    !stream_type.eq_ignore_ascii_case("time")
}

pub(super) fn map_category_to_string(category: &EventCategory) -> String {
    category.as_str().to_string()
}
