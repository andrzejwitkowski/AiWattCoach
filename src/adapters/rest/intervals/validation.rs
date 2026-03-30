use std::str::FromStr;

use axum::http::StatusCode;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

use crate::domain::intervals::{EventCategory, EventFileUpload};

use super::dto::EventFileUploadDto;

pub const MAX_ACTIVITY_UPLOAD_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_ACTIVITY_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

impl TryFrom<EventFileUploadDto> for EventFileUpload {
    type Error = ();

    fn try_from(value: EventFileUploadDto) -> Result<Self, Self::Error> {
        let file_contents = normalize_optional_upload_field(value.file_contents);
        let file_contents_base64 = normalize_optional_upload_field(value.file_contents_base64);

        if file_contents.is_some() == file_contents_base64.is_some() {
            return Err(());
        }

        Ok(EventFileUpload {
            filename: value.filename,
            file_contents,
            file_contents_base64,
        })
    }
}

pub(super) fn try_map_event_file_upload(
    file_upload: Option<EventFileUploadDto>,
) -> Result<Option<EventFileUpload>, StatusCode> {
    file_upload
        .map(EventFileUpload::try_from)
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)
}

pub(super) fn decode_base64(value: &str) -> Result<Vec<u8>, ()> {
    let clean: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    if clean.is_empty() || !clean.len().is_multiple_of(4) {
        return Err(());
    }

    let chunk_count = clean.len() / 4;
    for (chunk_index, chunk) in clean.as_bytes().chunks(4).enumerate() {
        let is_last = chunk_index + 1 == chunk_count;
        let padding_positions: Vec<usize> = chunk
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'=').then_some(index))
            .collect();

        if !padding_positions.is_empty() {
            if !is_last {
                return Err(());
            }

            match padding_positions.as_slice() {
                [3] | [2, 3] => {}
                _ => return Err(()),
            }
        }
    }

    let decoded = BASE64_STANDARD.decode(clean).map_err(|_| ())?;
    if decoded.len() > MAX_ACTIVITY_UPLOAD_BYTES {
        return Err(());
    }

    Ok(decoded)
}

pub(super) fn parse_category(category: &str) -> Option<EventCategory> {
    EventCategory::from_str(category).ok()
}

pub(super) fn is_valid_date(date: &str) -> bool {
    let mut segments = date.split('-');
    let (Some(year), Some(month), Some(day), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };

    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.chars().all(|ch| ch.is_ascii_digit())
        || !month.chars().all(|ch| ch.is_ascii_digit())
        || !day.chars().all(|ch| ch.is_ascii_digit())
    {
        return false;
    }

    let Ok(year) = year.parse::<i32>() else {
        return false;
    };
    let Ok(month) = month.parse::<u32>() else {
        return false;
    };
    let Ok(day) = day.parse::<u32>() else {
        return false;
    };

    let is_leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year => 29,
        2 => 28,
        _ => return false,
    };

    (1..=max_day).contains(&day)
}

fn normalize_optional_upload_field(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    })
}
