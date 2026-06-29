use serde::Serialize;

use super::super::header_table::{
    cell_bool, cell_f64, cell_i32, cell_i64, cell_opt_f64, cell_opt_i32, cell_opt_json,
    cell_opt_str, cell_opt_u16, cell_str, interval_blocks_table, uniform_string, HeaderTable,
    TableBuilder,
};
use crate::domain::training_context::model::{
    AthleteProfileContext, FuturePlannedEventContext, HistoricalTrainingContext,
    HistoricalWorkoutContext, IntervalsStatusContext, PlannedRestDayContext, RaceContext,
    TrainingContext,
};

#[derive(Serialize)]
pub(crate) struct StablePayload {
    v: u8,
    i: CompactIntervalsStatus,
    p: CompactProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    rc: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fe: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prd: Option<HeaderTable>,
    h: CompactHistory,
}

impl StablePayload {
    pub(crate) fn from_context(context: &TrainingContext) -> Self {
        Self {
            v: 3,
            i: CompactIntervalsStatus::from_status(&context.intervals_status),
            p: CompactProfile::from_profile(&context.profile),
            rc: build_race_table(&context.races),
            fe: build_future_events_table(&context.future_events),
            prd: build_planned_rest_days_table(&context.planned_rest_days),
            h: CompactHistory::from_history(&context.history),
        }
    }
}

fn build_race_table(races: &[RaceContext]) -> Option<HeaderTable> {
    if races.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("d", false),
        ("n", false),
        ("km", false),
        ("pri", false),
        ("id", false),
    ]);
    for race in races {
        builder = builder.push_row(vec![
            cell_str(&race.date),
            cell_str(&race.name),
            cell_f64(race.distance_meters as f64 / 1000.0),
            cell_str(&race.priority),
            cell_str(&race.race_id),
        ]);
    }
    if let Some(disc) = uniform_string(races.iter().map(|race| race.discipline.as_str())) {
        builder = builder.def_str("def_disc", &disc);
    }
    builder.build()
}

fn build_future_events_table(events: &[FuturePlannedEventContext]) -> Option<HeaderTable> {
    if events.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("id", false),
        ("sd", false),
        ("c", false),
        ("ty", true),
        ("n", true),
        ("desc", true),
        ("dur", true),
        ("tss", true),
        ("ifv", true),
        ("np", true),
    ]);
    for event in events {
        builder = builder.push_row(vec![
            cell_i64(event.event_id),
            cell_str(&event.start_date_local),
            cell_str(&event.category),
            cell_opt_str(event.event_type.as_deref()),
            cell_opt_str(event.name.as_deref()),
            cell_opt_str(event.description.as_deref()),
            cell_opt_i32(event.estimated_duration_seconds),
            cell_opt_f64(event.estimated_training_stress_score),
            cell_opt_f64(event.estimated_intensity_factor),
            cell_opt_i32(event.estimated_normalized_power_watts),
        ]);
    }
    builder.build()
}

fn build_planned_rest_days_table(entries: &[PlannedRestDayContext]) -> Option<HeaderTable> {
    if entries.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("id", false),
        ("sd", false),
        ("ed", false),
        ("n", true),
        ("nt", true),
    ]);
    for entry in entries {
        builder = builder.push_row(vec![
            cell_str(&entry.planned_rest_day_id),
            cell_str(&entry.start_date),
            cell_str(&entry.end_date),
            cell_opt_str(entry.title.as_deref()),
            cell_opt_str(entry.note.as_deref()),
        ]);
    }
    builder.build()
}

fn build_load_trend_table(
    points: &[crate::domain::training_context::model::HistoricalLoadTrendPoint],
) -> Option<HeaderTable> {
    if points.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[("d", false), ("tss", false)]);
    for point in points {
        builder = builder.push_row(vec![cell_str(&point.date), cell_i32(point.period_tss)]);
    }
    builder.build()
}

fn build_historical_workouts_table(workouts: &[HistoricalWorkoutContext]) -> Option<HeaderTable> {
    if workouts.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("d", false),
        ("id", false),
        ("n", true),
        ("dur", true),
        ("tss", true),
        ("np", true),
        ("ftp", true),
        ("recap", true),
        ("bl", true),
    ]);
    for workout in workouts {
        builder = builder.push_row(vec![
            cell_str(&workout.date),
            cell_str(&workout.activity_id),
            cell_opt_str(workout.name.as_deref()),
            cell_opt_i32(workout.duration_seconds),
            cell_opt_i32(workout.training_stress_score),
            cell_opt_i32(workout.normalized_power_watts),
            cell_opt_i32(workout.ftp_watts),
            cell_opt_str(workout.workout_recap.as_deref()),
            cell_opt_json(interval_blocks_table(&workout.interval_blocks)),
        ]);
    }
    if let Some(ty) = uniform_string(
        workouts
            .iter()
            .filter_map(|workout| workout.activity_type.as_deref()),
    ) {
        builder = builder.def_str("def_ty", &ty);
    }
    builder.build()
}

fn build_availability_table(
    days: &[crate::domain::training_context::model::WeeklyAvailabilityContext],
) -> Option<HeaderTable> {
    if days.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[("wd", false), ("a", false), ("mdm", true)]);
    for day in days {
        builder = builder.push_row(vec![
            cell_str(day.weekday.as_str()),
            cell_bool(day.available),
            cell_opt_u16(day.max_duration_minutes),
        ]);
    }
    builder.build()
}

#[derive(Serialize)]
struct CompactIntervalsStatus {
    a: String,
    e: String,
}

impl CompactIntervalsStatus {
    fn from_status(status: &IntervalsStatusContext) -> Self {
        Self {
            a: status.activities.clone(),
            e: status.events.clone(),
        }
    }
}

#[derive(Serialize)]
struct CompactProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    fnm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    age: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hcm: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wkg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ftp: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hrm: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vo2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meds: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    acfg: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    av: Option<HeaderTable>,
}

impl CompactProfile {
    fn from_profile(profile: &AthleteProfileContext) -> Self {
        Self {
            fnm: profile.full_name.clone(),
            age: profile.age,
            hcm: profile.height_cm,
            wkg: profile.weight_kg,
            ftp: profile.ftp_watts,
            hrm: profile.hr_max_bpm,
            vo2: profile.vo2_max,
            ap: profile.athlete_prompt.clone(),
            meds: profile.medications.clone(),
            notes: profile.athlete_notes.clone(),
            acfg: profile.availability_configured,
            av: build_availability_table(&profile.weekly_availability),
        }
    }
}

#[derive(Serialize)]
struct CompactHistory {
    ws: String,
    we: String,
    ac: usize,
    ttss: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    ctl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tsb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ftp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ftpd: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    t7: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    t28: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    if28: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ef28: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lt: Option<HeaderTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<HeaderTable>,
}

impl CompactHistory {
    fn from_history(history: &HistoricalTrainingContext) -> Self {
        Self {
            ws: history.window_start.clone(),
            we: history.window_end.clone(),
            ac: history.activity_count,
            ttss: history.total_tss,
            ctl: history.ctl,
            atl: history.atl,
            tsb: history.tsb,
            ftp: history.ftp_current,
            ftpd: history.ftp_change,
            t7: history.average_tss_7d,
            t28: history.average_tss_28d,
            if28: history.average_if_28d,
            ef28: history.average_ef_28d,
            lt: build_load_trend_table(&history.load_trend),
            w: build_historical_workouts_table(&history.workouts),
        }
    }
}
