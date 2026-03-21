use mongodb::bson::DateTime;

use crate::domain::identity::IdentityError;

pub fn epoch_seconds_to_bson_datetime(epoch_seconds: i64) -> Result<DateTime, IdentityError> {
    let epoch_millis = epoch_seconds.checked_mul(1000).ok_or_else(|| {
        IdentityError::Repository("expires_at timestamp exceeds BSON DateTime range".to_string())
    })?;

    Ok(DateTime::from_millis(epoch_millis))
}

#[cfg(test)]
mod tests {
    use crate::domain::identity::IdentityError;

    use super::epoch_seconds_to_bson_datetime;

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
}
