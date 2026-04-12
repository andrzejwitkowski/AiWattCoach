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
