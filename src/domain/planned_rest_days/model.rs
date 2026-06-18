use chrono::{Duration, NaiveDate};

pub const MAX_PLANNED_REST_DAY_RANGE_DAYS: i64 = 366;
pub const MAX_PLANNED_REST_DAY_TITLE_CHARS: usize = 120;
pub const MAX_PLANNED_REST_DAY_NOTE_CHARS: usize = 2000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedRestDayError {
    NotFound,
    Unauthenticated,
    Validation(String),
    Internal(String),
}

impl std::fmt::Display for PlannedRestDayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Planned rest day not found"),
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::Validation(message) | Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PlannedRestDayError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedRestDay {
    pub planned_rest_day_id: String,
    pub user_id: String,
    pub start_date: String,
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatePlannedRestDay {
    pub start_date: String,
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePlannedRestDay {
    pub start_date: String,
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
}

impl PlannedRestDay {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        planned_rest_day_id: String,
        user_id: String,
        start_date: String,
        end_date: String,
        title: Option<String>,
        note: Option<String>,
        created_at_epoch_seconds: i64,
        updated_at_epoch_seconds: i64,
    ) -> Result<Self, PlannedRestDayError> {
        validate_date_range(&start_date, &end_date)?;
        validate_optional_text(title.as_deref(), MAX_PLANNED_REST_DAY_TITLE_CHARS, "title")?;
        validate_optional_text(note.as_deref(), MAX_PLANNED_REST_DAY_NOTE_CHARS, "note")?;

        Ok(Self {
            planned_rest_day_id,
            user_id,
            start_date,
            end_date,
            title,
            note,
            created_at_epoch_seconds,
            updated_at_epoch_seconds,
        })
    }

    pub fn pending_new(
        planned_rest_day_id: String,
        user_id: String,
        request: CreatePlannedRestDay,
        now_epoch_seconds: i64,
    ) -> Result<Self, PlannedRestDayError> {
        Self::new(
            planned_rest_day_id,
            user_id,
            request.start_date,
            request.end_date,
            normalize_optional_text(request.title),
            normalize_optional_text(request.note),
            now_epoch_seconds,
            now_epoch_seconds,
        )
    }

    pub fn mark_updated(
        &self,
        request: UpdatePlannedRestDay,
        now_epoch_seconds: i64,
    ) -> Result<Self, PlannedRestDayError> {
        Self::new(
            self.planned_rest_day_id.clone(),
            self.user_id.clone(),
            request.start_date,
            request.end_date,
            normalize_optional_text(request.title),
            normalize_optional_text(request.note),
            self.created_at_epoch_seconds,
            now_epoch_seconds,
        )
    }

    pub fn default_label_title(&self) -> &str {
        "Planned rest"
    }

    pub fn display_title(&self) -> String {
        self.title
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(self.default_label_title())
            .to_string()
    }

    pub fn label_subtitle_for_date(&self, date: &str) -> Option<String> {
        if self.start_date != self.end_date && date == self.start_date {
            return Some(format!("{} – {}", self.start_date, self.end_date));
        }

        self.note.clone()
    }
}

pub fn parse_date(date: &str) -> Result<NaiveDate, PlannedRestDayError> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|error| PlannedRestDayError::Validation(format!("invalid date '{date}': {error}")))
}

pub fn validate_date_range(start_date: &str, end_date: &str) -> Result<(), PlannedRestDayError> {
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;

    if end < start {
        return Err(PlannedRestDayError::Validation(
            "end date must be on or after start date".to_string(),
        ));
    }

    let span_days = (end - start).num_days() + 1;
    if span_days > MAX_PLANNED_REST_DAY_RANGE_DAYS {
        return Err(PlannedRestDayError::Validation(format!(
            "planned rest day range cannot exceed {MAX_PLANNED_REST_DAY_RANGE_DAYS} days"
        )));
    }

    Ok(())
}

pub fn validate_write_range_ends_on_or_after(
    today: NaiveDate,
    end_date: &str,
) -> Result<(), PlannedRestDayError> {
    let end = parse_date(end_date)?;
    if end < today {
        return Err(PlannedRestDayError::Validation(
            "planned rest day range must include today or a future date".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_past_date_changes_allowed(
    existing: &PlannedRestDay,
    request: &UpdatePlannedRestDay,
    today: NaiveDate,
) -> Result<(), PlannedRestDayError> {
    let existing_end = parse_date(&existing.end_date)?;
    if existing_end >= today {
        return Ok(());
    }

    if existing.start_date != request.start_date || existing.end_date != request.end_date {
        return Err(PlannedRestDayError::Validation(
            "cannot change dates for a fully past planned rest day range".to_string(),
        ));
    }

    Ok(())
}

pub fn expand_inclusive_date_range(
    start_date: &str,
    end_date: &str,
) -> Result<Vec<String>, PlannedRestDayError> {
    validate_date_range(start_date, end_date)?;
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    let span_days = (end - start).num_days() + 1;
    let mut dates = Vec::with_capacity(span_days as usize);
    let mut current = start;

    while current <= end {
        dates.push(current.format("%Y-%m-%d").to_string());
        current += Duration::days(1);
    }

    Ok(dates)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn validate_optional_text(
    value: Option<&str>,
    max_chars: usize,
    field_name: &str,
) -> Result<(), PlannedRestDayError> {
    let Some(value) = value else {
        return Ok(());
    };

    if value.chars().count() > max_chars {
        return Err(PlannedRestDayError::Validation(format!(
            "{field_name} cannot exceed {max_chars} characters"
        )));
    }

    Ok(())
}
