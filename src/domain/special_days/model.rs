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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecialDayError {
    Repository(String),
}

impl std::fmt::Display for SpecialDayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
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
    ) -> Self {
        Self {
            special_day_id,
            user_id,
            date,
            kind,
        }
    }
}
