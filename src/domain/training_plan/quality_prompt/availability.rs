use chrono::{Datelike, Duration, NaiveDate};

pub fn format_plan_quality_availability(
    today: &str,
    availability_configured: bool,
    weekly_availability: &[crate::domain::training_context::WeeklyAvailabilityContext],
    day_count: i64,
) -> String {
    if !availability_configured {
        return "availability: not configured".to_string();
    }

    let Some(today_date) = NaiveDate::parse_from_str(today, "%Y-%m-%d").ok() else {
        return "availability: not configured".to_string();
    };

    let by_weekday: std::collections::HashMap<_, _> = weekly_availability
        .iter()
        .map(|day| (day.weekday, day))
        .collect();

    let mut parts = Vec::new();
    for offset in 1..=day_count {
        let date = today_date + Duration::days(offset);
        let settings_weekday = chrono_weekday_to_settings(date.weekday());
        let label = settings_weekday_label(settings_weekday);
        let date_key = date.format("%Y-%m-%d").to_string();
        let part = match by_weekday.get(&settings_weekday) {
            Some(day) if day.available => match day.max_duration_minutes {
                Some(max) => format!("{date_key} available max={max}m ({label})"),
                None => format!("{date_key} available ({label})"),
            },
            _ => format!("{date_key} unavailable ({label})"),
        };
        parts.push(part);
    }

    format!("availability: {}", parts.join("; "))
}

fn chrono_weekday_to_settings(weekday: chrono::Weekday) -> crate::domain::settings::Weekday {
    use crate::domain::settings::Weekday as S;
    use chrono::Weekday as C;
    match weekday {
        C::Mon => S::Mon,
        C::Tue => S::Tue,
        C::Wed => S::Wed,
        C::Thu => S::Thu,
        C::Fri => S::Fri,
        C::Sat => S::Sat,
        C::Sun => S::Sun,
    }
}

fn settings_weekday_label(weekday: crate::domain::settings::Weekday) -> &'static str {
    use crate::domain::settings::Weekday as S;
    match weekday {
        S::Mon => "Monday",
        S::Tue => "Tuesday",
        S::Wed => "Wednesday",
        S::Thu => "Thursday",
        S::Fri => "Friday",
        S::Sat => "Saturday",
        S::Sun => "Sunday",
    }
}
