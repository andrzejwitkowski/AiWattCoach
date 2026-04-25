use std::io::Cursor;

use fitparser::{
    profile::field_types::{MesgNum, Sport, SubSport},
    FitDataRecord, Value,
};

use crate::domain::{
    completed_workouts::{
        CompletedWorkoutDetails, CompletedWorkoutInterval, CompletedWorkoutMetrics,
        CompletedWorkoutSeries, CompletedWorkoutStream,
    },
    wahoo_fit_enrichment::{BoxFuture, ParsedWahooFitWorkout, WahooFitParserPort},
};

#[derive(Clone, Default)]
pub struct WahooFitParser;

impl WahooFitParserPort for WahooFitParser {
    fn parse_fit_workout(
        &self,
        file_bytes: &[u8],
    ) -> BoxFuture<Result<ParsedWahooFitWorkout, String>> {
        let file_bytes = file_bytes.to_vec();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || parse_fit_workout(&file_bytes))
                .await
                .map_err(|error| format!("FIT parse task failed: {error}"))?
        })
    }
}

fn parse_fit_workout(file_bytes: &[u8]) -> Result<ParsedWahooFitWorkout, String> {
    let mut cursor = Cursor::new(file_bytes);
    let records = fitparser::from_reader(&mut cursor)
        .map_err(|error| format!("failed to decode FIT file: {error}"))?;
    let session = records
        .iter()
        .find(|record| record.kind() == MesgNum::Session)
        .ok_or_else(|| "FIT file did not contain a session record".to_string())?;

    let laps = records
        .iter()
        .filter(|record| record.kind() == MesgNum::Lap)
        .cloned()
        .collect::<Vec<_>>();
    let record_messages = records
        .iter()
        .filter(|record| record.kind() == MesgNum::Record)
        .cloned()
        .collect::<Vec<_>>();

    let average_power_watts = int_field(session, "avg_power");
    let normalized_power_watts = int_field(session, "normalized_power");

    Ok(ParsedWahooFitWorkout {
        duration_seconds: int_field(session, "total_elapsed_time")
            .or_else(|| int_field(session, "total_timer_time")),
        distance_meters: float_field(session, "total_distance"),
        activity_type: map_activity_type(
            enum_u8_field(session, "sport").map(Sport::from),
            enum_u8_field(session, "sub_sport").map(SubSport::from),
        ),
        trainer: is_trainer_activity(
            enum_u8_field(session, "sport").map(Sport::from),
            enum_u8_field(session, "sub_sport").map(SubSport::from),
        ),
        metrics: CompletedWorkoutMetrics {
            training_stress_score: int_field(session, "training_stress_score"),
            normalized_power_watts,
            intensity_factor: float_field(session, "intensity_factor"),
            efficiency_factor: None,
            variability_index: normalized_power_watts.zip(average_power_watts).and_then(
                |(normalized, average)| (average > 0).then_some(normalized as f64 / average as f64),
            ),
            average_power_watts,
            ftp_watts: int_field(session, "threshold_power"),
            total_work_joules: int_field(session, "total_work"),
            calories: int_field(session, "total_calories"),
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: CompletedWorkoutDetails {
            intervals: parse_lap_intervals(&laps),
            interval_groups: Vec::new(),
            streams: parse_streams(&record_messages),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    })
}

fn parse_lap_intervals(laps: &[FitDataRecord]) -> Vec<CompletedWorkoutInterval> {
    let mut cumulative_elapsed_seconds: i32 = 0;
    laps.iter()
        .enumerate()
        .map(|(index, lap)| {
            let elapsed_time_seconds =
                int_field(lap, "total_elapsed_time").or_else(|| int_field(lap, "total_timer_time"));
            let moving_time_seconds =
                int_field(lap, "total_timer_time").or_else(|| int_field(lap, "total_elapsed_time"));
            let start_time_seconds = Some(cumulative_elapsed_seconds);
            let end_time_seconds = elapsed_time_seconds.map(|elapsed| {
                cumulative_elapsed_seconds = cumulative_elapsed_seconds.saturating_add(elapsed);
                cumulative_elapsed_seconds
            });
            CompletedWorkoutInterval {
                id: Some((index + 1) as i32),
                label: Some(format!("Lap {}", index + 1)),
                interval_type: Some("lap".to_string()),
                group_id: None,
                start_index: None,
                end_index: None,
                start_time_seconds,
                end_time_seconds,
                moving_time_seconds,
                elapsed_time_seconds,
                distance_meters: float_field(lap, "total_distance"),
                average_power_watts: int_field(lap, "avg_power"),
                normalized_power_watts: int_field(lap, "normalized_power"),
                training_stress_score: float_field(lap, "training_stress_score"),
                average_heart_rate_bpm: int_field(lap, "avg_heart_rate"),
                average_cadence_rpm: float_field(lap, "avg_cadence"),
                average_speed_mps: float_field(lap, "enhanced_avg_speed")
                    .or_else(|| float_field(lap, "avg_speed")),
                average_stride_meters: None,
                zone: None,
            }
        })
        .collect()
}

fn parse_streams(records: &[FitDataRecord]) -> Vec<CompletedWorkoutStream> {
    let watts = integer_stream(records, "power");
    let cadence = integer_stream(records, "cadence");
    let heart_rate = integer_stream(records, "heart_rate");
    let distance = float_stream(records, &["distance"]);
    let speed = float_stream(records, &["enhanced_speed", "speed"]);

    [
        stream_from_i64_values("watts", "Power", watts),
        stream_from_i64_values("cadence", "Cadence", cadence),
        stream_from_i64_values("heartrate", "Heart Rate", heart_rate),
        stream_from_f64_values("distance", "Distance", distance),
        stream_from_f64_values("velocity_smooth", "Speed", speed),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn integer_stream(records: &[FitDataRecord], field_name: &str) -> Vec<i64> {
    records
        .iter()
        .filter_map(|record| int_field(record, field_name).map(i64::from))
        .collect()
}

fn float_stream(records: &[FitDataRecord], field_names: &[&str]) -> Vec<f64> {
    records
        .iter()
        .filter_map(|record| {
            field_names
                .iter()
                .find_map(|field_name| float_field(record, field_name))
        })
        .collect()
}

fn stream_from_i64_values(
    stream_type: &str,
    name: &str,
    values: Vec<i64>,
) -> Option<CompletedWorkoutStream> {
    if values.is_empty() {
        return None;
    }

    Some(CompletedWorkoutStream {
        stream_type: stream_type.to_string(),
        name: Some(name.to_string()),
        primary_series: Some(CompletedWorkoutSeries::Integers(values)),
        secondary_series: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    })
}

fn stream_from_f64_values(
    stream_type: &str,
    name: &str,
    values: Vec<f64>,
) -> Option<CompletedWorkoutStream> {
    if values.is_empty() {
        return None;
    }

    Some(CompletedWorkoutStream {
        stream_type: stream_type.to_string(),
        name: Some(name.to_string()),
        primary_series: Some(CompletedWorkoutSeries::Floats(values)),
        secondary_series: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    })
}

fn enum_u8_field(record: &FitDataRecord, field_name: &str) -> Option<u8> {
    let value = record
        .fields()
        .iter()
        .find(|field| field.name() == field_name)?
        .value();
    match value {
        Value::Enum(value) | Value::UInt8(value) | Value::UInt8z(value) | Value::Byte(value) => {
            Some(*value)
        }
        _ => None,
    }
}

fn int_field(record: &FitDataRecord, field_name: &str) -> Option<i32> {
    let value = record
        .fields()
        .iter()
        .find(|field| field.name() == field_name)?
        .value();
    match value {
        Value::UInt8(value) | Value::UInt8z(value) | Value::Byte(value) | Value::Enum(value) => {
            Some(i32::from(*value))
        }
        Value::UInt16(value) | Value::UInt16z(value) => Some(i32::from(*value)),
        Value::UInt32(value) | Value::UInt32z(value) => i32::try_from(*value).ok(),
        Value::SInt8(value) => Some(i32::from(*value)),
        Value::SInt16(value) => Some(i32::from(*value)),
        Value::SInt32(value) => Some(*value),
        Value::Float32(value) => Some(value.round() as i32),
        Value::Float64(value) => Some(value.round() as i32),
        _ => None,
    }
}

fn float_field(record: &FitDataRecord, field_name: &str) -> Option<f64> {
    let value = record
        .fields()
        .iter()
        .find(|field| field.name() == field_name)?
        .value();
    match value {
        Value::Float32(value) => Some(f64::from(*value)),
        Value::Float64(value) => Some(*value),
        Value::UInt8(value) | Value::UInt8z(value) | Value::Byte(value) | Value::Enum(value) => {
            Some(f64::from(*value))
        }
        Value::UInt16(value) | Value::UInt16z(value) => Some(f64::from(*value)),
        Value::UInt32(value) | Value::UInt32z(value) => Some(f64::from(*value)),
        Value::SInt8(value) => Some(f64::from(*value)),
        Value::SInt16(value) => Some(f64::from(*value)),
        Value::SInt32(value) => Some(f64::from(*value)),
        _ => None,
    }
}

fn map_activity_type(sport: Option<Sport>, sub_sport: Option<SubSport>) -> Option<String> {
    match (sport?, sub_sport) {
        (Sport::Cycling, _) | (Sport::Generic, Some(SubSport::VirtualActivity)) => {
            Some("Ride".to_string())
        }
        (Sport::Running, _) => Some("Run".to_string()),
        (Sport::Walking, _) | (Sport::Hiking, _) => Some("Walk".to_string()),
        (Sport::Swimming, _) => Some("Swim".to_string()),
        _ => None,
    }
}

fn is_trainer_activity(sport: Option<Sport>, sub_sport: Option<SubSport>) -> Option<bool> {
    match (sport?, sub_sport) {
        (
            Sport::Cycling,
            Some(SubSport::VirtualActivity | SubSport::IndoorCycling | SubSport::Spin),
        ) => Some(true),
        (Sport::Generic, Some(SubSport::VirtualActivity)) => Some(true),
        _ => Some(false),
    }
}
