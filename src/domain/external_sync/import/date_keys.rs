const CALENDAR_DATE_PREFIX_LEN: usize = 10;
const START_MINUTE_BUCKET_PREFIX_LEN: usize = 16;

pub(super) fn date_key(value: &str) -> &str {
    value.get(..CALENDAR_DATE_PREFIX_LEN).unwrap_or(value)
}

pub(super) fn start_minute_bucket(value: &str) -> Option<String> {
    value
        .get(..START_MINUTE_BUCKET_PREFIX_LEN)
        .map(ToString::to_string)
}
