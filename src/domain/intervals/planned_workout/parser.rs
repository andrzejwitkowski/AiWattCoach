use chrono::NaiveDate;

use super::{
    PlannedWorkout, PlannedWorkoutDay, PlannedWorkoutDays, PlannedWorkoutLine,
    PlannedWorkoutParseError, PlannedWorkoutRepeat, PlannedWorkoutStep, PlannedWorkoutStepKind,
    PlannedWorkoutTarget, PlannedWorkoutText,
};

pub fn parse_planned_workout(input: &str) -> Result<PlannedWorkout, PlannedWorkoutParseError> {
    let mut lines = Vec::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(step) = parse_step(line)? {
            lines.push(PlannedWorkoutLine::Step(step));
            continue;
        }

        if let Some(repeat) = parse_repeat(line)? {
            lines.push(PlannedWorkoutLine::Repeat(repeat));
            continue;
        }

        lines.push(PlannedWorkoutLine::Text(PlannedWorkoutText {
            text: normalize_spaces(line),
        }));
    }

    Ok(PlannedWorkout { lines })
}

pub fn parse_planned_workout_days(
    input: &str,
) -> Result<PlannedWorkoutDays, PlannedWorkoutParseError> {
    let mut days = Vec::new();
    let mut current_date: Option<String> = None;
    let mut current_lines = Vec::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if is_exact_date(line) {
            if let Some(date) = current_date.take() {
                days.push(parse_day(&date, &current_lines)?);
                current_lines.clear();
            }

            current_date = Some(line.to_string());
            continue;
        }

        if current_date.is_none() {
            return Err(PlannedWorkoutParseError::new(
                "content before first date header",
            ));
        }

        current_lines.push(line.to_string());
    }

    if let Some(date) = current_date {
        days.push(parse_day(&date, &current_lines)?);
    }

    Ok(PlannedWorkoutDays { days })
}

pub fn serialize_planned_workout(workout: &PlannedWorkout) -> String {
    workout
        .lines
        .iter()
        .map(serialize_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_day(date: &str, lines: &[String]) -> Result<PlannedWorkoutDay, PlannedWorkoutParseError> {
    if lines.is_empty() {
        return Err(PlannedWorkoutParseError::new(format!(
            "failed to parse day {date}: day body is empty"
        )));
    }

    if lines.len() == 1 && lines[0].eq_ignore_ascii_case("rest day") {
        return Ok(PlannedWorkoutDay::rest(date.to_string()));
    }

    let workout_input = lines.join("\n");
    let workout = parse_planned_workout(&workout_input).map_err(|error| {
        PlannedWorkoutParseError::new(format!("failed to parse day {date}: {error}"))
    })?;

    Ok(PlannedWorkoutDay::workout(date.to_string(), workout))
}

fn parse_step(line: &str) -> Result<Option<PlannedWorkoutStep>, PlannedWorkoutParseError> {
    let Some(step_body) = line.strip_prefix('-') else {
        return Ok(None);
    };

    let normalized = normalize_spaces(step_body.trim());
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let Some(duration_seconds) = tokens.first().and_then(|token| parse_duration_token(token))
    else {
        return Err(PlannedWorkoutParseError::new(format!(
            "invalid planned workout step: {line}"
        )));
    };

    let mut index = 1;
    let kind = if tokens
        .get(index)
        .is_some_and(|token| token.eq_ignore_ascii_case("ramp"))
    {
        index += 1;
        PlannedWorkoutStepKind::Ramp
    } else {
        PlannedWorkoutStepKind::Steady
    };

    let target = tokens
        .get(index)
        .and_then(|token| parse_target(token))
        .ok_or_else(|| {
            PlannedWorkoutParseError::new(format!("invalid planned workout step: {line}"))
        })?;

    if tokens.len() != index + 1 {
        return Err(PlannedWorkoutParseError::new(format!(
            "invalid planned workout step: {line}"
        )));
    }

    Ok(Some(PlannedWorkoutStep {
        duration_seconds,
        kind,
        target,
    }))
}

fn parse_repeat(line: &str) -> Result<Option<PlannedWorkoutRepeat>, PlannedWorkoutParseError> {
    let normalized = normalize_spaces(line);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let Some(last) = tokens.last() else {
        return Ok(None);
    };
    let Some(count_text) = last.strip_suffix('x') else {
        return Ok(None);
    };
    if count_text.is_empty() || !count_text.chars().all(|ch| ch.is_ascii_digit()) {
        return Ok(None);
    }

    let count = count_text
        .parse::<usize>()
        .map_err(|_| PlannedWorkoutParseError::new(format!("invalid repeat header: {line}")))?;
    if count == 0 {
        return Err(PlannedWorkoutParseError::new(format!(
            "invalid repeat header: {line}"
        )));
    }

    let title = if tokens.len() > 1 {
        Some(tokens[..tokens.len() - 1].join(" "))
    } else {
        None
    };

    Ok(Some(PlannedWorkoutRepeat { title, count }))
}

fn parse_duration_token(token: &str) -> Option<i32> {
    let token = token.trim().to_ascii_lowercase();
    let split_index = token
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(token.len());
    let (value, unit) = token.split_at(split_index);
    if value.is_empty() || unit.is_empty() {
        return None;
    }

    let value = value.parse::<i32>().ok()?;
    if value <= 0 {
        return None;
    }

    match unit {
        "m" | "min" => value.checked_mul(60),
        "s" => Some(value),
        _ => None,
    }
}

fn parse_target(token: &str) -> Option<PlannedWorkoutTarget> {
    parse_percent_target(token).or_else(|| parse_watts_target(token))
}

fn parse_percent_target(token: &str) -> Option<PlannedWorkoutTarget> {
    let token = token.trim();
    let raw = token.strip_suffix('%')?;
    let (min, max) = if let Some((min, max)) = raw.split_once('-') {
        let min = min.parse::<f64>().ok()?;
        let max = max.parse::<f64>().ok()?;
        (min.min(max), min.max(max))
    } else {
        let value = raw.parse::<f64>().ok()?;
        (value, value)
    };
    if !min.is_finite() || !max.is_finite() || min <= 0.0 || max <= 0.0 {
        return None;
    }

    Some(PlannedWorkoutTarget::PercentFtp { min, max })
}

fn parse_watts_target(token: &str) -> Option<PlannedWorkoutTarget> {
    let raw = token
        .trim()
        .strip_suffix('W')
        .or_else(|| token.trim().strip_suffix('w'))?;
    let (min, max) = raw.split_once('-')?;
    let min = min.parse::<i32>().ok()?;
    let max = max.parse::<i32>().ok()?;
    if min <= 0 || max <= 0 {
        return None;
    }
    Some(PlannedWorkoutTarget::WattsRange {
        min: min.min(max),
        max: min.max(max),
    })
}

fn serialize_line(line: &PlannedWorkoutLine) -> String {
    match line {
        PlannedWorkoutLine::Text(text) => text.text.clone(),
        PlannedWorkoutLine::Repeat(repeat) => match &repeat.title {
            Some(title) => format!("{title} {}x", repeat.count),
            None => format!("{}x", repeat.count),
        },
        PlannedWorkoutLine::Step(step) => {
            let duration = serialize_duration(step.duration_seconds);
            let target = serialize_target(&step.target);
            match step.kind {
                PlannedWorkoutStepKind::Steady => format!("- {duration} {target}"),
                PlannedWorkoutStepKind::Ramp => format!("- {duration} ramp {target}"),
            }
        }
    }
}

fn serialize_duration(duration_seconds: i32) -> String {
    if duration_seconds % 60 == 0 {
        let duration_minutes = duration_seconds / 60;
        format!("{duration_minutes}m")
    } else {
        format!("{duration_seconds}s")
    }
}

fn serialize_target(target: &PlannedWorkoutTarget) -> String {
    match target {
        PlannedWorkoutTarget::PercentFtp { min, max } => {
            if (min - max).abs() < f64::EPSILON {
                format_number(*min) + "%"
            } else {
                format!("{}-{}%", format_number(*min), format_number(*max))
            }
        }
        PlannedWorkoutTarget::WattsRange { min, max } => format!("{min}-{max}W"),
    }
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        (value as i32).to_string()
    } else {
        value.to_string()
    }
}

fn normalize_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_exact_date(value: &str) -> bool {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}
