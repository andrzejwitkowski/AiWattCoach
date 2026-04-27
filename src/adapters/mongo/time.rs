use mongodb::bson::DateTime;

use crate::domain::identity::IdentityError;

pub fn epoch_seconds_to_bson_datetime(epoch_seconds: i64) -> Result<DateTime, IdentityError> {
    epoch_seconds_to_bson_datetime_with_field(epoch_seconds, "expires_at")
        .map_err(IdentityError::Repository)
}

pub fn epoch_seconds_to_bson_datetime_with_field(
    epoch_seconds: i64,
    field_name: &str,
) -> Result<DateTime, String> {
    let epoch_millis = epoch_seconds
        .checked_mul(1000)
        .ok_or_else(|| bson_range_error(field_name))?;

    Ok(DateTime::from_millis(epoch_millis))
}

pub fn optional_epoch_seconds_to_bson_datetime(
    epoch_seconds: Option<i64>,
    field_name: &str,
) -> Result<Option<DateTime>, String> {
    epoch_seconds
        .map(|epoch_seconds| epoch_seconds_to_bson_datetime_with_field(epoch_seconds, field_name))
        .transpose()
}

pub fn required_epoch_seconds_to_bson_datetime(
    epoch_seconds: i64,
    field_name: &str,
) -> Option<DateTime> {
    Some(
        epoch_seconds_to_bson_datetime_with_field(epoch_seconds, field_name)
            .unwrap_or_else(|error| panic!("{error}")),
    )
}

pub fn bson_datetime_to_epoch_seconds(datetime: DateTime) -> i64 {
    datetime.timestamp_millis().div_euclid(1000)
}

pub fn optional_bson_datetime_to_epoch_seconds(datetime: Option<DateTime>) -> Option<i64> {
    datetime.map(bson_datetime_to_epoch_seconds)
}

pub fn resolve_required_epoch_seconds(
    datetime: Option<DateTime>,
    epoch_seconds: Option<i64>,
    field_name: &str,
) -> Result<i64, String> {
    resolve_optional_epoch_seconds(datetime, epoch_seconds)
        .ok_or_else(|| format!("missing {field_name} timestamp"))
}

pub fn resolve_optional_epoch_seconds(
    datetime: Option<DateTime>,
    epoch_seconds: Option<i64>,
) -> Option<i64> {
    optional_bson_datetime_to_epoch_seconds(datetime).or(epoch_seconds)
}

fn bson_range_error(field_name: &str) -> String {
    format!("{field_name} timestamp exceeds BSON DateTime range")
}

#[cfg(test)]
mod tests {
    use mongodb::bson::DateTime;

    use crate::domain::identity::IdentityError;

    use super::{
        bson_datetime_to_epoch_seconds, epoch_seconds_to_bson_datetime,
        epoch_seconds_to_bson_datetime_with_field, optional_bson_datetime_to_epoch_seconds,
        optional_epoch_seconds_to_bson_datetime, required_epoch_seconds_to_bson_datetime,
        resolve_optional_epoch_seconds, resolve_required_epoch_seconds,
    };

    #[test]
    fn converts_epoch_seconds_to_bson_datetime() {
        let datetime = epoch_seconds_to_bson_datetime(1_700_000_000).unwrap();

        assert_eq!(datetime.timestamp_millis(), 1_700_000_000_000);
    }

    #[test]
    fn rejects_epoch_seconds_that_overflow_bson_millis() {
        let error = epoch_seconds_to_bson_datetime(i64::MAX / 1000 + 1).unwrap_err();

        assert!(
            matches!(error, IdentityError::Repository(message) if message.contains("expires_at timestamp exceeds BSON DateTime range"))
        );
    }

    #[test]
    fn converts_bson_datetime_back_to_epoch_seconds() {
        let datetime = epoch_seconds_to_bson_datetime_with_field(1_700_000_123, "updated_at")
            .expect("datetime should convert");

        assert_eq!(bson_datetime_to_epoch_seconds(datetime), 1_700_000_123);
    }

    #[test]
    fn converts_optional_epoch_seconds_to_bson_datetime() {
        let datetime = optional_epoch_seconds_to_bson_datetime(Some(1_700_000_000), "saved_at")
            .expect("datetime should convert");

        assert_eq!(
            optional_bson_datetime_to_epoch_seconds(datetime),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn converts_missing_optional_epoch_seconds_to_none() {
        let datetime =
            optional_epoch_seconds_to_bson_datetime(None, "saved_at").expect("none should map");

        assert_eq!(datetime, None);
        assert_eq!(optional_bson_datetime_to_epoch_seconds(None), None);
    }

    #[test]
    fn resolves_required_epoch_seconds_from_datetime() {
        let datetime = epoch_seconds_to_bson_datetime_with_field(1_700_000_000, "updated_at")
            .expect("datetime should convert");

        let epoch_seconds = resolve_required_epoch_seconds(Some(datetime), None, "updated_at")
            .expect("time should resolve");

        assert_eq!(epoch_seconds, 1_700_000_000);
    }

    #[test]
    fn resolves_optional_epoch_seconds_preferring_datetime() {
        let datetime = epoch_seconds_to_bson_datetime_with_field(1_700_000_100, "updated_at")
            .expect("datetime should convert");

        let epoch_seconds = resolve_optional_epoch_seconds(Some(datetime), Some(1_700_000_000));

        assert_eq!(epoch_seconds, Some(1_700_000_100));
    }

    #[test]
    fn converts_required_epoch_seconds_to_bson_datetime() {
        let datetime = required_epoch_seconds_to_bson_datetime(1_700_000_000, "created_at");

        assert_eq!(
            optional_bson_datetime_to_epoch_seconds(datetime),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn converts_negative_bson_datetime_back_to_epoch_seconds_with_flooring() {
        let datetime = DateTime::from_millis(-1);

        assert_eq!(bson_datetime_to_epoch_seconds(datetime), -1);
    }
}
