use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use super::{
    ast::{
        CadenceRange, ParserTarget, RepeatBlockAst, StepAmount, StepKind, WorkoutAst, WorkoutItem,
        WorkoutStepAst,
    },
    error::WorkoutPestParseError,
};

#[derive(Parser)]
#[grammar = "domain/intervals/workout/pest_parser/workout.pest"]
struct WorkoutDocParser;

pub fn parse_workout_ast(input: &str) -> Result<WorkoutAst, WorkoutPestParseError> {
    let mut pairs = WorkoutDocParser::parse(Rule::workout, input)
        .map_err(|error| WorkoutPestParseError::new(format!("invalid workout syntax: {error}")))?;
    let workout = pairs
        .next()
        .ok_or_else(|| WorkoutPestParseError::new("missing workout document"))?;
    let mut items = Vec::new();
    let mut pending_repeat: Option<RepeatBlockAst> = None;

    for pair in workout.into_inner() {
        match pair.as_rule() {
            Rule::blank_line => {
                push_pending_repeat(&mut items, &mut pending_repeat)?;
            }
            Rule::step_line => {
                let step = parse_step_line(pair)?;
                if let Some(repeat) = pending_repeat.as_mut() {
                    repeat.steps.push(step);
                } else {
                    items.push(WorkoutItem::Step(step));
                }
            }
            Rule::repeat_header_line => {
                push_pending_repeat(&mut items, &mut pending_repeat)?;
                pending_repeat = Some(parse_repeat_header(pair)?);
            }
            Rule::text_line => {
                push_pending_repeat(&mut items, &mut pending_repeat)?;
            }
            _ => {}
        }
    }

    push_pending_repeat(&mut items, &mut pending_repeat)?;

    Ok(WorkoutAst { items })
}

fn push_pending_repeat(
    items: &mut Vec<WorkoutItem>,
    pending_repeat: &mut Option<RepeatBlockAst>,
) -> Result<(), WorkoutPestParseError> {
    let Some(repeat) = pending_repeat.take() else {
        return Ok(());
    };

    if repeat.steps.is_empty() {
        return Err(WorkoutPestParseError::new(
            "repeat block must include at least one step",
        ));
    }

    items.push(WorkoutItem::RepeatBlock(repeat));
    Ok(())
}

fn parse_step_line(step_line: Pair<'_, Rule>) -> Result<WorkoutStepAst, WorkoutPestParseError> {
    let raw = step_line.as_str().trim();
    let step_body = step_line
        .into_inner()
        .find(|pair| pair.as_rule() == Rule::step_body)
        .ok_or_else(|| WorkoutPestParseError::new("missing step body"))?;
    parse_step_body(step_body, raw)
}

fn parse_repeat_header(
    header_line: Pair<'_, Rule>,
) -> Result<RepeatBlockAst, WorkoutPestParseError> {
    let mut title = None;
    let mut count = None;

    for pair in header_line.into_inner() {
        match pair.as_rule() {
            Rule::repeat_title => {
                let value = pair.as_str().trim();
                if !value.is_empty() {
                    title = Some(value.to_string());
                }
            }
            Rule::repeat_count => {
                let parsed = pair
                    .as_str()
                    .trim_end_matches('x')
                    .parse::<usize>()
                    .map_err(|_| WorkoutPestParseError::new("invalid repeat count"))?;
                if parsed == 0 {
                    return Err(WorkoutPestParseError::new("invalid repeat count"));
                }
                count = Some(parsed);
            }
            _ => {}
        }
    }

    Ok(RepeatBlockAst {
        title,
        count: count.ok_or_else(|| WorkoutPestParseError::new("missing repeat count"))?,
        steps: Vec::new(),
    })
}

fn parse_step_body(
    step_body: Pair<'_, Rule>,
    original_line: &str,
) -> Result<WorkoutStepAst, WorkoutPestParseError> {
    let mut cue = None;
    let mut amount = None;
    let mut kind = StepKind::Steady;
    let mut target = None;
    let mut cadence_rpm = None;
    let mut text = None;

    for pair in step_body.into_inner() {
        match pair.as_rule() {
            Rule::cue_prefix => {
                let value = pair.as_str().trim();
                if !value.is_empty() {
                    cue = Some(value.to_string());
                }
            }
            Rule::amount => {
                amount = Some(parse_amount_pair(pair)?);
            }
            Rule::modifier => {
                kind = parse_kind_pair(pair)?;
            }
            Rule::target => {
                target = Some(parse_target_pair(pair)?);
            }
            Rule::cadence => {
                cadence_rpm = Some(parse_cadence_pair(pair)?);
            }
            Rule::text_metadata => {
                text = Some(parse_text_metadata_pair(pair)?);
            }
            _ => {}
        }
    }

    let amount = amount.ok_or_else(|| {
        WorkoutPestParseError::new(format!("invalid workout syntax: {original_line}"))
    })?;

    Ok(WorkoutStepAst {
        cue,
        amount,
        kind,
        target,
        cadence_rpm,
        text,
        raw: original_line
            .trim()
            .trim_start_matches('-')
            .trim()
            .to_string(),
    })
}

fn parse_time_amount(value: &str) -> Result<StepAmount, WorkoutPestParseError> {
    let lower = value.to_ascii_lowercase();
    let (raw_amount, unit) = if let Some(amount) = lower.strip_suffix("mins") {
        (amount, "mins")
    } else if let Some(amount) = lower.strip_suffix("min") {
        (amount, "min")
    } else if let Some(amount) = lower.strip_suffix('m') {
        (amount, "m")
    } else if let Some(amount) = lower.strip_suffix("hrs") {
        (amount, "hrs")
    } else if let Some(amount) = lower.strip_suffix("hr") {
        (amount, "hr")
    } else if let Some(amount) = lower.strip_suffix('h') {
        (amount, "h")
    } else if let Some(amount) = lower.strip_suffix("secs") {
        (amount, "secs")
    } else if let Some(amount) = lower.strip_suffix("sec") {
        (amount, "sec")
    } else if let Some(amount) = lower.strip_suffix('s') {
        (amount, "s")
    } else {
        return Err(WorkoutPestParseError::new("unsupported time unit"));
    };

    let amount = raw_amount
        .parse::<i32>()
        .map_err(|_| WorkoutPestParseError::new("invalid time amount"))?;

    let minutes = match unit {
        "mins" | "min" | "m" => amount,
        "hrs" | "hr" | "h" => amount
            .checked_mul(60)
            .ok_or_else(|| WorkoutPestParseError::new("invalid time amount"))?,
        "secs" | "sec" | "s" => {
            if amount <= 0 {
                return Err(WorkoutPestParseError::new(
                    "time amount in seconds must be positive",
                ));
            }
            ((amount - 1) / 60) + 1
        }
        _ => return Err(WorkoutPestParseError::new("unsupported time unit")),
    };

    Ok(StepAmount::DurationMinutes(minutes))
}

fn parse_distance_amount(value: &str) -> Result<StepAmount, WorkoutPestParseError> {
    let lower = value.to_ascii_lowercase();
    let kilometers = if let Some(amount) = lower.strip_suffix("km") {
        amount
            .parse::<f64>()
            .map_err(|_| WorkoutPestParseError::new("invalid distance amount"))?
    } else if let Some(amount) = lower.strip_suffix("mtr") {
        amount
            .parse::<f64>()
            .map_err(|_| WorkoutPestParseError::new("invalid distance amount"))?
            / 1000.0
    } else if let Some(amount) = lower.strip_suffix("mi") {
        amount
            .parse::<f64>()
            .map_err(|_| WorkoutPestParseError::new("invalid distance amount"))?
            * 1.609_344
    } else {
        return Err(WorkoutPestParseError::new("unsupported distance unit"));
    };

    Ok(StepAmount::DistanceKilometers(kilometers))
}

fn parse_kind_pair(pair: Pair<'_, Rule>) -> Result<StepKind, WorkoutPestParseError> {
    let modifier = pair
        .into_inner()
        .next()
        .ok_or_else(|| WorkoutPestParseError::new("missing step modifier"))?;

    match modifier.as_rule() {
        Rule::ramp => Ok(StepKind::Ramp),
        Rule::freeride => Ok(StepKind::FreeRide),
        _ => Err(WorkoutPestParseError::new("invalid step modifier")),
    }
}

fn parse_target_pair(pair: Pair<'_, Rule>) -> Result<ParserTarget, WorkoutPestParseError> {
    let target = pair
        .into_inner()
        .next()
        .ok_or_else(|| WorkoutPestParseError::new("missing target"))?;

    match target.as_rule() {
        Rule::ftp_target => parse_percent_target(target.as_str(), PercentTargetKind::Ftp),
        Rule::hr_target => parse_percent_target(target.as_str(), PercentTargetKind::Hr),
        Rule::lthr_target => parse_percent_target(target.as_str(), PercentTargetKind::Lthr),
        Rule::pace_target => parse_pace_target(target),
        _ => Err(WorkoutPestParseError::new("invalid target")),
    }
}

fn parse_cadence_pair(pair: Pair<'_, Rule>) -> Result<CadenceRange, WorkoutPestParseError> {
    let lower = pair.as_str().to_ascii_lowercase();
    let raw = lower
        .strip_suffix("rpm")
        .ok_or_else(|| WorkoutPestParseError::new("invalid cadence value"))?;
    if let Some((start, end)) = raw.split_once('-') {
        let start = start
            .parse::<i32>()
            .map_err(|_| WorkoutPestParseError::new("invalid cadence value"))?;
        let end = end
            .parse::<i32>()
            .map_err(|_| WorkoutPestParseError::new("invalid cadence value"))?;
        return Ok(CadenceRange {
            min_rpm: start.min(end),
            max_rpm: start.max(end),
        });
    }

    let cadence = raw
        .parse::<i32>()
        .map_err(|_| WorkoutPestParseError::new("invalid cadence value"))?;
    Ok(CadenceRange {
        min_rpm: cadence,
        max_rpm: cadence,
    })
}

fn parse_text_metadata_pair(pair: Pair<'_, Rule>) -> Result<String, WorkoutPestParseError> {
    let quoted = pair
        .into_inner()
        .find(|inner| inner.as_rule() == Rule::quoted_string)
        .ok_or_else(|| WorkoutPestParseError::new("missing text metadata"))?;
    let raw = quoted.as_str();
    let unquoted = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| WorkoutPestParseError::new("invalid text metadata"))?;

    Ok(unquoted.replace("\\\"", "\""))
}

enum PercentTargetKind {
    Ftp,
    Hr,
    Lthr,
}

fn parse_percent_target(
    value: &str,
    kind: PercentTargetKind,
) -> Result<ParserTarget, WorkoutPestParseError> {
    let lower = value.to_ascii_lowercase();
    let raw = lower
        .trim()
        .trim_end_matches("lthr")
        .trim_end_matches("hr")
        .trim()
        .trim_end_matches('%');
    let (min, max) = if let Some((start, end)) = raw.split_once('-') {
        let start = start
            .trim()
            .parse::<f64>()
            .map_err(|_| WorkoutPestParseError::new("invalid percent target"))?;
        let end = end
            .trim()
            .parse::<f64>()
            .map_err(|_| WorkoutPestParseError::new("invalid percent target"))?;
        (start.min(end), start.max(end))
    } else {
        let value = raw
            .parse::<f64>()
            .map_err(|_| WorkoutPestParseError::new("invalid percent target"))?;
        (value, value)
    };

    let target = match kind {
        PercentTargetKind::Ftp => ParserTarget::PercentFtp { min, max },
        PercentTargetKind::Hr => ParserTarget::PercentHr { min, max },
        PercentTargetKind::Lthr => ParserTarget::PercentLthr { min, max },
    };

    Ok(target)
}

fn parse_pace_target(pair: Pair<'_, Rule>) -> Result<ParserTarget, WorkoutPestParseError> {
    let pace_value = pair
        .into_inner()
        .find(|inner| inner.as_rule() == Rule::pace_value)
        .ok_or_else(|| WorkoutPestParseError::new("missing pace value"))?;

    Ok(ParserTarget::Pace {
        value: pace_value.as_str().to_string(),
    })
}

fn parse_amount_pair(pair: Pair<'_, Rule>) -> Result<StepAmount, WorkoutPestParseError> {
    let amount = pair
        .into_inner()
        .next()
        .ok_or_else(|| WorkoutPestParseError::new("missing amount"))?;

    match amount.as_rule() {
        Rule::distance_amount => parse_distance_amount(amount.as_str()),
        Rule::time_amount => parse_time_amount(amount.as_str()),
        _ => Err(WorkoutPestParseError::new("unsupported amount")),
    }
}
