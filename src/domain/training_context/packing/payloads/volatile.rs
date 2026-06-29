use chrono::{Duration, NaiveDate};
use serde::Serialize;
use serde_json::{json, Value};

use super::super::header_table::{
    cell_bool, cell_i32, cell_i64, cell_opt_f64, cell_opt_i32, cell_opt_json, cell_opt_segments,
    cell_opt_str, cell_opt_u8, cell_opt_value, cell_str, interval_blocks_table, HeaderTable,
    TableBuilder,
};
use crate::domain::training_context::model::{
    PlannedWorkoutContext, PlannedWorkoutReference, ProjectedDayContext, ProjectedWorkoutContext,
    RaceContext, RecentDayContext, RecentWorkoutContext, RecentWorkoutRecapContext,
    SpecialDayContext, TrainingContext, UpcomingDayContext,
};

const RACE_STRATEGY_WINDOW_DAYS: i64 = 14;

#[derive(Serialize)]
pub(crate) struct VolatilePayload<'a> {
    v: u8,
    g: i64,
    fx: CompactFocus<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rd: Vec<CompactRecentDay<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wr: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ud: Vec<CompactUpcomingDay<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pd: Vec<CompactProjectedDay<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rs: Option<HeaderTable>,
}

impl<'a> VolatilePayload<'a> {
    pub(crate) fn from_context(context: &'a TrainingContext) -> Self {
        Self {
            v: 3,
            g: context.generated_at_epoch_seconds,
            fx: CompactFocus {
                id: context.focus_workout_id.as_deref(),
                k: &context.focus_kind,
            },
            rd: context
                .recent_days
                .iter()
                .map(CompactRecentDay::from_recent_day)
                .collect(),
            wr: build_workout_recaps_table(&context.recent_workout_recaps),
            ud: context
                .upcoming_days
                .iter()
                .map(CompactUpcomingDay::from_upcoming_day)
                .collect(),
            pd: context
                .projected_days
                .iter()
                .map(CompactProjectedDay::from_projected_day)
                .collect(),
            rs: build_race_strategy_table(context),
        }
    }
}

fn build_workout_recaps_table(recaps: &[RecentWorkoutRecapContext]) -> Option<HeaderTable> {
    if recaps.is_empty() {
        return None;
    }
    let mut builder =
        TableBuilder::new(&[("d", false), ("id", false), ("rpe", true), ("recap", false)]);
    for recap in recaps {
        builder = builder.push_row(vec![
            cell_str(&recap.date),
            cell_str(&recap.workout_id),
            cell_opt_u8(recap.rpe),
            cell_str(&recap.recap),
        ]);
    }
    builder.build()
}

fn build_race_strategy_table(context: &TrainingContext) -> Option<HeaderTable> {
    if context.races.is_empty() {
        return None;
    }
    let focus_date = infer_packed_focus_date(context)?;
    let window_end = focus_date + Duration::days(RACE_STRATEGY_WINDOW_DAYS);
    let mut rows = context
        .races
        .iter()
        .filter_map(|race| race_strategy_row(race, focus_date, window_end))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    rows.sort_by_key(|row| row.0);
    let mut builder = TableBuilder::new(&[
        ("d", false),
        ("pri", false),
        ("disc", false),
        ("n", false),
        ("days_out", false),
    ]);
    for (date, pri, disc, name, days_out) in rows {
        builder = builder.push_row(vec![
            cell_str(date),
            cell_str(pri),
            cell_str(disc),
            cell_str(name),
            cell_i32(days_out),
        ]);
    }
    builder.build()
}

fn race_strategy_row(
    race: &RaceContext,
    focus_date: NaiveDate,
    window_end: NaiveDate,
) -> Option<(&str, &str, &str, &str, i32)> {
    let race_date = NaiveDate::parse_from_str(&race.date, "%Y-%m-%d").ok()?;
    if race_date < focus_date || race_date > window_end {
        return None;
    }
    Some((
        &race.date,
        &race.priority,
        &race.discipline,
        &race.name,
        (race_date - focus_date).num_days() as i32,
    ))
}

fn infer_packed_focus_date(context: &TrainingContext) -> Option<NaiveDate> {
    if let Some(latest_recent_day) = context
        .recent_days
        .iter()
        .filter_map(|day| NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").ok())
        .max()
    {
        return Some(latest_recent_day);
    }
    if let Some(earliest_upcoming_day) = context
        .upcoming_days
        .iter()
        .filter_map(|day| NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").ok())
        .min()
    {
        return earliest_upcoming_day.pred_opt();
    }
    if let Some(earliest_projected_day) = context
        .projected_days
        .iter()
        .filter_map(|day| NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").ok())
        .min()
    {
        return earliest_projected_day.pred_opt();
    }
    chrono::DateTime::from_timestamp(context.generated_at_epoch_seconds, 0)
        .map(|timestamp| timestamp.date_naive())
}

fn planned_workout_ref_json(reference: &PlannedWorkoutReference) -> Value {
    let mut value = json!({
        "id": reference.event_id,
        "sd": reference.start_date_local,
        "c": reference.category,
        "done": reference.completed,
    });
    if let Some(name) = reference.name.as_deref() {
        value["n"] = json!(name);
    }
    if let Some(doc) = reference.raw_workout_doc.as_deref() {
        value["doc"] = json!(doc);
    }
    if let Some(tss) = reference.estimated_training_stress_score {
        value["tss"] = json!(tss);
    }
    if let Some(ifv) = reference.estimated_intensity_factor {
        value["ifv"] = json!(ifv);
    }
    if let Some(np) = reference.estimated_normalized_power_watts {
        value["np"] = json!(np);
    }
    if let Some(bl) = interval_blocks_table(&reference.interval_blocks) {
        if let Ok(bl) = serde_json::to_value(bl) {
            value["bl"] = bl;
        }
    }
    value
}

fn build_recent_workouts_table(workouts: &[RecentWorkoutContext]) -> Option<HeaderTable> {
    if workouts.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("id", false),
        ("sd", false),
        ("n", true),
        ("tss", true),
        ("np", true),
        ("ftp", true),
        ("rpe", true),
        ("recap", true),
        ("ps", true),
        ("cs", true),
        ("pw", true),
    ]);
    for workout in workouts {
        builder = builder.push_row(vec![
            cell_str(&workout.activity_id),
            cell_str(&workout.start_date_local),
            cell_opt_str(workout.name.as_deref()),
            cell_opt_i32(workout.training_stress_score),
            cell_opt_i32(workout.normalized_power_watts),
            cell_opt_i32(workout.ftp_watts),
            cell_opt_u8(workout.rpe),
            cell_opt_str(workout.workout_recap.as_deref()),
            cell_opt_segments(&workout.power_segments),
            cell_opt_segments(&workout.cadence_segments),
            cell_opt_value(
                workout
                    .planned_workout
                    .as_ref()
                    .map(planned_workout_ref_json),
            ),
        ]);
    }
    builder.build()
}

fn build_planned_workouts_table(planned: &[PlannedWorkoutContext]) -> Option<HeaderTable> {
    if planned.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("id", false),
        ("sd", false),
        ("n", true),
        ("c", false),
        ("bl", true),
        ("doc", true),
        ("tss", true),
        ("ifv", true),
        ("np", true),
        ("done", false),
    ]);
    for workout in planned {
        builder = builder.push_row(vec![
            cell_i64(workout.event_id),
            cell_str(&workout.start_date_local),
            cell_opt_str(workout.name.as_deref()),
            cell_str(&workout.category),
            cell_opt_json(interval_blocks_table(&workout.interval_blocks)),
            cell_opt_str(workout.raw_workout_doc.as_deref()),
            cell_opt_f64(workout.estimated_training_stress_score),
            cell_opt_f64(workout.estimated_intensity_factor),
            cell_opt_i32(workout.estimated_normalized_power_watts),
            cell_bool(workout.completed),
        ]);
    }
    builder.build()
}

fn build_special_days_table(special_days: &[SpecialDayContext]) -> Option<HeaderTable> {
    if special_days.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("id", false),
        ("sd", false),
        ("n", true),
        ("c", false),
        ("desc", true),
    ]);
    for special in special_days {
        builder = builder.push_row(vec![
            cell_i64(special.event_id),
            cell_str(&special.start_date_local),
            cell_opt_str(special.name.as_deref()),
            cell_str(&special.category),
            cell_opt_str(special.description.as_deref()),
        ]);
    }
    builder.build()
}

fn build_projected_workouts_table(workouts: &[ProjectedWorkoutContext]) -> Option<HeaderTable> {
    if workouts.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("swid", false),
        ("sd", false),
        ("n", true),
        ("bl", true),
        ("doc", true),
        ("rest", false),
        ("rr", true),
    ]);
    for workout in workouts {
        builder = builder.push_row(vec![
            cell_str(&workout.source_workout_id),
            cell_str(&workout.start_date_local),
            cell_opt_str(workout.name.as_deref()),
            cell_opt_json(interval_blocks_table(&workout.interval_blocks)),
            cell_opt_str(workout.raw_workout_doc.as_deref()),
            cell_bool(workout.rest_day),
            cell_opt_str(workout.rest_day_reason.as_deref()),
        ]);
    }
    builder.build()
}

#[derive(Serialize)]
struct CompactFocus<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    k: &'a str,
}

#[derive(Serialize)]
struct CompactRecentDay<'a> {
    d: &'a str,
    fr: bool,
    sick: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    sickn: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pw: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sd: Option<HeaderTable>,
}

impl<'a> CompactRecentDay<'a> {
    fn from_recent_day(day: &'a RecentDayContext) -> Self {
        Self {
            d: &day.date,
            fr: day.free_day,
            sick: day.sick_day,
            sickn: day.sick_note.as_deref(),
            w: build_recent_workouts_table(&day.workouts),
            pw: build_planned_workouts_table(&day.planned_workouts),
            sd: build_special_days_table(&day.special_days),
        }
    }
}

#[derive(Serialize)]
struct CompactUpcomingDay<'a> {
    d: &'a str,
    fr: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pw: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sd: Option<HeaderTable>,
}

impl<'a> CompactUpcomingDay<'a> {
    fn from_upcoming_day(day: &'a UpcomingDayContext) -> Self {
        Self {
            d: &day.date,
            fr: day.free_day,
            pw: build_planned_workouts_table(&day.planned_workouts),
            sd: build_special_days_table(&day.special_days),
        }
    }
}

#[derive(Serialize)]
struct CompactProjectedDay<'a> {
    d: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<HeaderTable>,
}

impl<'a> CompactProjectedDay<'a> {
    fn from_projected_day(day: &'a ProjectedDayContext) -> Self {
        Self {
            d: &day.date,
            w: build_projected_workouts_table(&day.workouts),
        }
    }
}
