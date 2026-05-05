use chrono::NaiveDate;
use serde::Serialize;
use serde_json::json;

use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutSeries},
    planned_workouts::PlannedWorkout,
    races::Race,
    workout_summary::{MessageRole, WorkoutSummary},
};

use super::parse_date;

#[derive(Serialize)]
pub(super) struct GetSelectedWorkoutResponse {
    date: String,
    workouts: Vec<WorkoutEntry>,
    races: Vec<RaceEntry>,
}

pub(super) struct SelectedDate {
    pub(super) value: String,
    pub(super) parsed: NaiveDate,
}

pub(super) struct SelectedWorkoutData {
    pub(super) completed: Vec<CompletedWorkout>,
    pub(super) planned: Vec<PlannedWorkout>,
    pub(super) races: Vec<Race>,
    pub(super) summaries: Vec<WorkoutSummary>,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum WorkoutEntry {
    #[serde(rename = "completed")]
    Completed {
        workout_id: String,
        name: Option<String>,
        start_date_local: String,
        duration_seconds: Option<i32>,
        distance_meters: Option<f64>,
        metrics: CompletedWorkoutMetricsDto,
        streams: Vec<StreamDto>,
        ai_conversation: Vec<ConversationMessageDto>,
        ai_summary: Option<String>,
    },
    #[serde(rename = "planned")]
    Planned {
        planned_workout_id: String,
        name: Option<String>,
        date: String,
        status: String,
        rest_day: bool,
        rest_day_reason: Option<String>,
        raw_workout_doc: Option<String>,
    },
}

#[derive(Serialize)]
struct CompletedWorkoutMetricsDto {
    training_stress_score: Option<i32>,
    normalized_power_watts: Option<i32>,
    intensity_factor: Option<f64>,
    efficiency_factor: Option<f64>,
    average_power_watts: Option<i32>,
    ftp_watts: Option<i32>,
}

#[derive(Serialize)]
struct StreamDto {
    stream_type: String,
    data: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_data: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
struct ConversationMessageDto {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct RaceEntry {
    race_id: String,
    name: String,
    date: String,
    distance_meters: i32,
    discipline: String,
    priority: String,
}

pub(super) fn build_selected_workout_response(
    date: String,
    data: SelectedWorkoutData,
    selected_date: &SelectedDate,
    today: &str,
) -> GetSelectedWorkoutResponse {
    let include_races = data.completed.is_empty();
    GetSelectedWorkoutResponse {
        date: date.clone(),
        workouts: build_workout_entries(&data, selected_date, today),
        races: build_race_entries(data.races, include_races, &date),
    }
}

fn build_workout_entries(
    data: &SelectedWorkoutData,
    selected_date: &SelectedDate,
    today: &str,
) -> Vec<WorkoutEntry> {
    let mut workouts: Vec<WorkoutEntry> = Vec::new();

    for workout in &data.completed {
        workouts.push(map_completed_workout(workout, &data.summaries));
    }

    for plan in &data.planned {
        let is_completed = data.completed.iter().any(|c| {
            c.planned_workout_id
                .as_ref()
                .is_some_and(|id| id == &plan.planned_workout_id)
        });
        if !is_completed {
            workouts.push(map_planned_workout(plan, selected_date, today));
        }
    }

    workouts
}

fn map_completed_workout(workout: &CompletedWorkout, summaries: &[WorkoutSummary]) -> WorkoutEntry {
    let summary = summaries
        .iter()
        .find(|s| s.workout_id == workout.completed_workout_id);

    WorkoutEntry::Completed {
        workout_id: workout.completed_workout_id.clone(),
        name: workout.name.clone(),
        start_date_local: workout.start_date_local.clone(),
        duration_seconds: workout.duration_seconds,
        distance_meters: workout.distance_meters,
        metrics: CompletedWorkoutMetricsDto {
            training_stress_score: workout.metrics.training_stress_score,
            normalized_power_watts: workout.metrics.normalized_power_watts,
            intensity_factor: workout.metrics.intensity_factor,
            efficiency_factor: workout.metrics.efficiency_factor,
            average_power_watts: workout.metrics.average_power_watts,
            ftp_watts: workout.metrics.ftp_watts,
        },
        streams: workout
            .details
            .streams
            .iter()
            .map(|stream| StreamDto {
                stream_type: stream.stream_type.clone(),
                data: series_values(stream.primary_series.as_ref()),
                secondary_data: stream
                    .secondary_series
                    .as_ref()
                    .map(|series| series_values(Some(series))),
            })
            .collect(),
        ai_conversation: summary
            .map(|s| s.messages.iter().map(map_conversation_message).collect())
            .unwrap_or_default(),
        ai_summary: summary.and_then(|s| s.workout_recap_text.clone()),
    }
}

fn map_conversation_message(
    message: &crate::domain::workout_summary::ConversationMessage,
) -> ConversationMessageDto {
    ConversationMessageDto {
        role: match message.role {
            MessageRole::User => "user".to_string(),
            MessageRole::Coach => "coach".to_string(),
            MessageRole::Tool => "tool".to_string(),
        },
        content: message.content.clone(),
    }
}

fn series_values(series: Option<&CompletedWorkoutSeries>) -> Vec<serde_json::Value> {
    match series {
        Some(CompletedWorkoutSeries::Integers(v)) => v.iter().map(|x| json!(x)).collect(),
        Some(CompletedWorkoutSeries::Floats(v)) => v.iter().map(|x| json!(x)).collect(),
        Some(CompletedWorkoutSeries::Bools(v)) => v.iter().map(|x| json!(x)).collect(),
        Some(CompletedWorkoutSeries::Strings(v)) => v.iter().map(|x| json!(x)).collect(),
        None => Vec::new(),
    }
}

fn map_planned_workout(
    plan: &PlannedWorkout,
    selected_date: &SelectedDate,
    today: &str,
) -> WorkoutEntry {
    WorkoutEntry::Planned {
        planned_workout_id: plan.planned_workout_id.clone(),
        name: plan.name.clone(),
        date: plan.date.clone(),
        status: planned_status(selected_date, today).to_string(),
        rest_day: plan.rest_day,
        rest_day_reason: plan.rest_day_reason.clone(),
        raw_workout_doc: (!plan.rest_day).then(|| serialize_planned_workout(&plan.workout.lines)),
    }
}

fn planned_status(selected_date: &SelectedDate, today: &str) -> &'static str {
    match parse_date(today) {
        Some(today) if selected_date.parsed < today => "not_completed",
        _ => "planned",
    }
}

fn build_race_entries(races: Vec<Race>, include_races: bool, date: &str) -> Vec<RaceEntry> {
    if !include_races {
        return Vec::new();
    }

    races
        .into_iter()
        .filter(|race| race.date == date)
        .map(|race| RaceEntry {
            race_id: race.race_id,
            name: race.name,
            date: race.date,
            distance_meters: race.distance_meters,
            discipline: race.discipline.as_str().to_string(),
            priority: race.priority.as_str().to_string(),
        })
        .collect()
}

fn serialize_planned_workout(
    lines: &[crate::domain::planned_workouts::PlannedWorkoutLine],
) -> String {
    let mapped: Vec<crate::domain::intervals::PlannedWorkoutLine> =
        lines.iter().map(map_planned_workout_line).collect();

    crate::domain::intervals::serialize_planned_workout(&crate::domain::intervals::PlannedWorkout {
        lines: mapped,
    })
}

fn map_planned_workout_line(
    line: &crate::domain::planned_workouts::PlannedWorkoutLine,
) -> crate::domain::intervals::PlannedWorkoutLine {
    match line {
        crate::domain::planned_workouts::PlannedWorkoutLine::BlankLine => {
            crate::domain::intervals::PlannedWorkoutLine::BlankLine
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Text(t) => {
            crate::domain::intervals::PlannedWorkoutLine::Text(
                crate::domain::intervals::PlannedWorkoutText {
                    text: t.text.clone(),
                },
            )
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Repeat(r) => {
            crate::domain::intervals::PlannedWorkoutLine::Repeat(
                crate::domain::intervals::PlannedWorkoutRepeat {
                    title: r.title.clone(),
                    count: r.count,
                },
            )
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Step(s) => {
            crate::domain::intervals::PlannedWorkoutLine::Step(map_planned_workout_step(s))
        }
    }
}

fn map_planned_workout_step(
    step: &crate::domain::planned_workouts::PlannedWorkoutStep,
) -> crate::domain::intervals::PlannedWorkoutStep {
    crate::domain::intervals::PlannedWorkoutStep {
        duration_seconds: step.duration_seconds,
        kind: match step.kind {
            crate::domain::planned_workouts::PlannedWorkoutStepKind::Steady => {
                crate::domain::intervals::PlannedWorkoutStepKind::Steady
            }
            crate::domain::planned_workouts::PlannedWorkoutStepKind::Ramp => {
                crate::domain::intervals::PlannedWorkoutStepKind::Ramp
            }
        },
        target: match step.target {
            crate::domain::planned_workouts::PlannedWorkoutTarget::PercentFtp { min, max } => {
                crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max }
            }
            crate::domain::planned_workouts::PlannedWorkoutTarget::WattsRange { min, max } => {
                crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max }
            }
        },
    }
}
