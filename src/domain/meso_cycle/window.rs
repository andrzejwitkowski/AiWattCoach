use chrono::{Duration, NaiveDate};

use super::{MesoCycleError, MesoCycleWindow, MESO_CYCLE_WINDOW_DAY_COUNT};

pub fn parse_date(value: &str) -> Result<NaiveDate, MesoCycleError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|error| MesoCycleError::Validation(format!("invalid date {value}: {error}")))
}

pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn resolve_meso_window(
    today: &str,
    ai_coach_last_date: Option<&str>,
    source_training_plan_operation_key: Option<String>,
) -> Result<MesoCycleWindow, MesoCycleError> {
    let today_date = parse_date(today)?;
    let tomorrow = today_date + Duration::days(1);
    let meso_start = if let Some(ai_last) = ai_coach_last_date {
        let ai_last_date = parse_date(ai_last)?;
        std::cmp::max(ai_last_date + Duration::days(1), tomorrow)
    } else {
        tomorrow
    };
    let meso_end = meso_start + Duration::days((MESO_CYCLE_WINDOW_DAY_COUNT as i64) - 1);

    Ok(MesoCycleWindow {
        meso_start: format_date(meso_start),
        meso_end: format_date(meso_end),
        ai_coach_last_date: ai_coach_last_date.map(ToString::to_string),
        source_training_plan_operation_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_window_after_ai_coach_plan() {
        let window = resolve_meso_window(
            "2026-06-05",
            Some("2026-06-19"),
            Some("training-plan:user:workout:1".to_string()),
        )
        .expect("window should resolve");

        assert_eq!(window.meso_start, "2026-06-20");
        assert_eq!(window.meso_end, "2026-07-19");
        assert_eq!(window.ai_coach_last_date.as_deref(), Some("2026-06-19"));
    }

    #[test]
    fn resolve_window_clamps_stale_ai_coach_end_to_tomorrow() {
        let window = resolve_meso_window("2026-06-05", Some("2026-05-01"), None)
            .expect("window should resolve");

        assert_eq!(window.meso_start, "2026-06-06");
        assert_eq!(window.meso_end, "2026-07-05");
    }

    #[test]
    fn resolve_window_without_ai_coach_plan_starts_tomorrow() {
        let window = resolve_meso_window("2026-06-05", None, None).expect("window should resolve");

        assert_eq!(window.meso_start, "2026-06-06");
        assert_eq!(window.meso_end, "2026-07-05");
        assert!(window.ai_coach_last_date.is_none());
    }
}
