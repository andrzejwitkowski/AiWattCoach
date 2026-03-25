use std::io::Cursor;

use fitparser::{profile::field_types::MesgNum, FitDataRecord, Value};

use crate::domain::intervals::{
    ActivityFallbackIdentity, ActivityFileIdentityExtractorPort, BoxFuture, IntervalsError,
    UploadActivity,
};

#[derive(Clone, Default)]
pub struct ActivityFileIdentityExtractor;

impl ActivityFileIdentityExtractorPort for ActivityFileIdentityExtractor {
    fn extract_identity(
        &self,
        upload: &UploadActivity,
    ) -> BoxFuture<Result<Option<ActivityFallbackIdentity>, IntervalsError>> {
        let upload = upload.clone();
        Box::pin(async move { Ok(extract_fit_identity(&upload.file_bytes)) })
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
    let activity_type_bucket = string_field(&session, "sport")
        .or_else(|| string_field(&session, "sub_sport"))?
        .trim()
        .to_ascii_lowercase();
    if activity_type_bucket.is_empty() {
        return None;
    }

    let duration_bucket_seconds = int_field(&session, "total_elapsed_time")
        .or_else(|| int_field(&session, "total_timer_time"))
        .map(|seconds| ((seconds + 15) / 30) * 30)
        .filter(|seconds| *seconds > 0)?;

    let distance_bucket_meters = float_field(&session, "total_distance")
        .filter(|meters| meters.is_finite() && *meters > 0.0)
        .map(|meters| ((meters / 100.0).round() * 100.0) as i32);

    let trainer = bool_field(&session, "sport_index").unwrap_or(false);

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
        Value::Timestamp(value) => Some(value.format("%Y-%m-%dT%H:%M").to_string()),
        _ => None,
    }
}

fn string_field(record: &FitDataRecord, field_name: &str) -> Option<String> {
    let value = record
        .fields()
        .iter()
        .find(|field| field.name() == field_name)?
        .value();
    match value {
        Value::String(value) => Some(value.clone()),
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

fn bool_field(record: &FitDataRecord, field_name: &str) -> Option<bool> {
    let value = record
        .fields()
        .iter()
        .find(|field| field.name() == field_name)?
        .value();
    match value {
        Value::UInt8(value) | Value::UInt8z(value) | Value::Byte(value) | Value::Enum(value) => {
            Some(*value > 0)
        }
        Value::UInt16(value) | Value::UInt16z(value) => Some(*value > 0),
        Value::UInt32(value) | Value::UInt32z(value) => Some(*value > 0),
        Value::SInt8(value) => Some(*value > 0),
        Value::SInt16(value) => Some(*value > 0),
        Value::SInt32(value) => Some(*value > 0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_bytes_return_none() {
        assert_eq!(extract_fit_identity(&[1, 2, 3, 4]), None);
    }
}
