use chrono::NaiveDate;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecialDayKind {
    Illness,
    Travel,
    Blocked,
    Note,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecialDay {
    pub special_day_id: String,
    pub user_id: String,
    pub date: String,
    pub kind: SpecialDayKind,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecialDayError {
    Validation(String),
    Repository(String),
}

impl std::fmt::Display for SpecialDayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) | Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SpecialDayError {}

impl SpecialDay {
    pub fn new(
        special_day_id: String,
        user_id: String,
        date: String,
        kind: SpecialDayKind,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<Self, SpecialDayError> {
        validate_date(&date)?;

        Ok(Self {
            special_day_id,
            user_id,
            date,
            kind,
            title,
            description,
        })
    }
}

fn validate_date(date: &str) -> Result<(), SpecialDayError> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|error| {
        SpecialDayError::Validation(format!("invalid special day date '{date}': {error}"))
    })?;
    Ok(())
}
