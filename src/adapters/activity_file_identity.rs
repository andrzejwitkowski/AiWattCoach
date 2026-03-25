use std::io::Cursor;

use chrono::Utc;
use fitparser::{
    profile::field_types::{MesgNum, Sport, SubSport},
    FitDataRecord, Value,
};

use crate::domain::intervals::{
    round_duration_bucket, ActivityFallbackIdentity, ActivityFileIdentityExtractorPort, BoxFuture,
    IntervalsError, UploadActivity,
};

#[derive(Clone, Default)]
pub struct ActivityFileIdentityExtractor;

impl ActivityFileIdentityExtractorPort for ActivityFileIdentityExtractor {
    fn extract_identity(
        &self,
        upload: &UploadActivity,
    ) -> BoxFuture<Result<Option<ActivityFallbackIdentity>, IntervalsError>> {
        let file_bytes = upload.file_bytes.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || extract_fit_identity(&file_bytes))
                .await
                .map_err(|error| {
                    IntervalsError::Internal(format!(
                        "activity identity extraction task failed: {error}"
                    ))
                })
        })
    }
}

fn extract_fit_identity(file_bytes: &[u8]) -> Option<ActivityFallbackIdentity> {
    if file_bytes.len() < 12 {
        return None;
    }

    let mut cursor = Cursor::new(file_bytes);
    let records = fitparser::from_reader(&mut cursor).ok()?;
    let session = records
        .into_iter()
        .find(|record| record.kind() == MesgNum::Session)?;

    let start_bucket = timestamp_field(&session, "start_time")?;
    let sport = enum_u8_field(&session, "sport").map(Sport::from)?;
    let sub_sport = enum_u8_field(&session, "sub_sport").map(SubSport::from);
    let (activity_type_bucket, trainer) = map_fit_activity_type(sport, sub_sport)?;

    let duration_bucket_seconds = int_field(&session, "total_elapsed_time")
        .filter(|seconds| *seconds > 0)
        .or_else(|| int_field(&session, "total_timer_time").filter(|seconds| *seconds > 0))
        .map(round_duration_bucket)?;

    let distance_bucket_meters = float_field(&session, "total_distance")
        .filter(|meters| meters.is_finite() && *meters > 0.0)
        .map(|meters| ((meters / 100.0).round() * 100.0) as i32);

    Some(ActivityFallbackIdentity {
        start_bucket,
        activity_type_bucket,
        duration_bucket_seconds,
        distance_bucket_meters,
        trainer,
    })
}

fn timestamp_field(record: &FitDataRecord, field_name: &str) -> Option<String> {
    let value = record
        .fields()
        .iter()
        .find(|field| field.name() == field_name)?
        .value();
    match value {
        Value::Timestamp(value) => Some(
            value
                .with_timezone(&Utc)
                .format("%Y-%m-%dT%H:%M")
                .to_string(),
        ),
        _ => None,
    }
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

fn map_fit_activity_type(sport: Sport, sub_sport: Option<SubSport>) -> Option<(String, bool)> {
    match (sport, sub_sport) {
        (
            Sport::Cycling,
            Some(SubSport::VirtualActivity | SubSport::IndoorCycling | SubSport::Spin),
        ) => Some(("virtualride".to_string(), true)),
        (Sport::Cycling, _) => Some(("ride".to_string(), false)),
        (Sport::Running, _) => Some(("run".to_string(), false)),
        (Sport::Swimming, _) => Some(("swim".to_string(), false)),
        (Sport::Walking, _) => Some(("walk".to_string(), false)),
        (Sport::Hiking, _) => Some(("hike".to_string(), false)),
        (Sport::Rowing, _) => Some(("row".to_string(), false)),
        (Sport::Generic, Some(SubSport::VirtualActivity)) => {
            Some(("virtualride".to_string(), true))
        }
        (sport, _) => {
            let value = sport.to_string().trim().to_ascii_lowercase();
            if value.is_empty() {
                None
            } else {
                Some((value, false))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[test]
    fn virtual_cycling_maps_to_virtualride_and_trainer() {
        assert_eq!(
            map_fit_activity_type(Sport::Cycling, Some(SubSport::VirtualActivity)),
            Some(("virtualride".to_string(), true))
        );
    }

    #[test]
    fn road_cycling_maps_to_ride() {
        assert_eq!(
            map_fit_activity_type(Sport::Cycling, Some(SubSport::Road)),
            Some(("ride".to_string(), false))
        );
    }

    #[test]
    fn timestamp_field_normalizes_to_utc_minute() {
        let mut record = FitDataRecord::new(MesgNum::Session);
        let value = chrono::DateTime::<Utc>::from_timestamp(1_711_262_800, 0)
            .unwrap()
            .with_timezone(&Local);
        record.push(fitparser::FitDataField::new(
            "start_time".to_string(),
            0,
            None,
            Value::Timestamp(value),
            String::new(),
        ));

        assert_eq!(
            timestamp_field(&record, "start_time"),
            Some("2024-03-24T06:46".to_string())
        );
    }

    #[test]
    fn unsupported_bytes_return_none() {
        assert_eq!(extract_fit_identity(&[1, 2, 3, 4]), None);
    }
}
