use aiwattcoach::domain::planned_rest_days::PlannedRestDay;

pub(crate) fn sample_planned_rest_day() -> PlannedRestDay {
    PlannedRestDay::new(
        "prd-1".to_string(),
        "user-1".to_string(),
        "2026-12-24".to_string(),
        "2026-12-26".to_string(),
        Some("Holiday".to_string()),
        Some("Family trip".to_string()),
        1,
        1,
    )
    .unwrap()
}
