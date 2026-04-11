use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use super::{
    ast::{
        ParserTarget, RepeatBlockAst, StepAmount, StepKind, WorkoutAst, WorkoutItem, WorkoutStepAst,
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
            Rule::blank_line => {}
            Rule::step_line => {
                let step = parse_step_line(pair)?;
                if let Some(repeat) = pending_repeat.as_mut() {
                    repeat.steps.push(step);
                } else {
                    items.push(WorkoutItem::Step(step));
                }
            }
            Rule::repeat_header_line => {
                if let Some(repeat) = pending_repeat.take() {
                    items.push(WorkoutItem::RepeatBlock(repeat));
                }
                pending_repeat = Some(parse_repeat_header(pair)?);
            }
            Rule::text_line => {
                if let Some(repeat) = pending_repeat.take() {
                    items.push(WorkoutItem::RepeatBlock(repeat));
                }
            }
            _ => {}
        }
    }

    if let Some(repeat) = pending_repeat {
        items.push(WorkoutItem::RepeatBlock(repeat));
    }

    Ok(WorkoutAst { items })
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
                count = Some(
                    pair.as_str()
                        .trim_end_matches('x')
                        .parse::<usize>()
                        .map_err(|_| WorkoutPestParseError::new("invalid repeat count"))?
                        .max(1),
                );
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
    let minutes = value
        .strip_suffix("mins")
        .or_else(|| value.strip_suffix("min"))
        .or_else(|| value.strip_suffix('m'))
        .ok_or_else(|| WorkoutPestParseError::new("unsupported time unit"))?
        .parse::<i32>()
        .map_err(|_| WorkoutPestParseError::new("invalid time amount"))?;

    Ok(StepAmount::DurationMinutes(minutes))
}

fn parse_distance_amount(value: &str) -> Result<StepAmount, WorkoutPestParseError> {
    let kilometers = value
        .strip_suffix("km")
        .ok_or_else(|| WorkoutPestParseError::new("unsupported distance unit"))?
        .parse::<f64>()
        .map_err(|_| WorkoutPestParseError::new("invalid distance amount"))?;

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

fn parse_cadence_pair(pair: Pair<'_, Rule>) -> Result<(i32, i32), WorkoutPestParseError> {
    let raw = pair
        .as_str()
        .trim_end_matches("rpm")
        .trim_end_matches("RPM");
    if let Some((start, end)) = raw.split_once('-') {
        let start = start
            .parse::<i32>()
            .map_err(|_| WorkoutPestParseError::new("invalid cadence value"))?;
        let end = end
            .parse::<i32>()
            .map_err(|_| WorkoutPestParseError::new("invalid cadence value"))?;
        return Ok((start.min(end), start.max(end)));
    }

    let cadence = raw
        .parse::<i32>()
        .map_err(|_| WorkoutPestParseError::new("invalid cadence value"))?;
    Ok((cadence, cadence))
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
    let raw = value
        .trim()
        .trim_end_matches("LTHR")
        .trim_end_matches("HR")
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
