use chrono::{Duration, NaiveDate};

use crate::domain::intervals::{Activity, Event};

pub(super) fn date_range_inclusive(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut current = start;
    while current <= end {
        dates.push(current);
        current += Duration::days(1);
    }
    dates
}

pub(super) fn activity_date(activity: &Activity) -> NaiveDate {
    parse_date(&date_key(&activity.start_date_local))
}

pub(super) fn event_date(event: &Event) -> NaiveDate {
    parse_date(&date_key(&event.start_date_local))
}

pub(super) fn date_key(value: &str) -> String {
    value.get(..10).unwrap_or(value).to_string()
}

pub(super) fn parse_date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::DateTime::UNIX_EPOCH.date_naive())
}

pub(super) fn epoch_seconds_to_date(epoch_seconds: i64) -> NaiveDate {
    chrono::DateTime::from_timestamp(epoch_seconds, 0)
        .map(|time| time.date_naive())
        .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH.date_naive())
}

pub(super) fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
