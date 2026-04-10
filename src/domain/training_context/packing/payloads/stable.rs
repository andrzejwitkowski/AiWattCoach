use serde::Serialize;

use crate::domain::training_context::model::{
    AthleteProfileContext, FuturePlannedEventContext, HistoricalLoadTrendPoint,
    HistoricalTrainingContext, HistoricalWorkoutContext, IntervalsStatusContext,
    PlannedWorkoutBlockContext, RaceContext, TrainingContext,
};

#[derive(Serialize)]
pub(crate) struct StablePayload<'a> {
    v: u8,
    i: CompactIntervalsStatus<'a>,
    p: CompactProfile<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rc: Vec<CompactRace<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fe: Vec<CompactFuturePlannedEvent<'a>>,
    h: CompactHistory<'a>,
}

impl<'a> StablePayload<'a> {
    pub(crate) fn from_context(context: &'a TrainingContext) -> Self {
        Self {
            v: 1,
            i: CompactIntervalsStatus::from_status(&context.intervals_status),
            p: CompactProfile::from_profile(&context.profile),
            rc: context.races.iter().map(CompactRace::from_race).collect(),
            fe: context
                .future_events
                .iter()
                .map(CompactFuturePlannedEvent::from_event)
                .collect(),
            h: CompactHistory::from_history(&context.history),
        }
    }
}

#[derive(Serialize)]
struct CompactRace<'a> {
    id: &'a str,
    d: &'a str,
    n: &'a str,
    km: f64,
    disc: &'a str,
    pri: &'a str,
}

impl<'a> CompactRace<'a> {
    fn from_race(race: &'a RaceContext) -> Self {
        Self {
            id: &race.race_id,
            d: &race.date,
            n: &race.name,
            km: race.distance_meters as f64 / 1000.0,
            disc: &race.discipline,
            pri: &race.priority,
        }
    }
}

#[derive(Serialize)]
struct CompactFuturePlannedEvent<'a> {
    id: i64,
    sd: &'a str,
    c: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ty: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    desc: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dur: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ifv: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    np: Option<i32>,
}

impl<'a> CompactFuturePlannedEvent<'a> {
    fn from_event(event: &'a FuturePlannedEventContext) -> Self {
        Self {
            id: event.event_id,
            sd: &event.start_date_local,
            c: &event.category,
            ty: event.event_type.as_deref(),
            n: event.name.as_deref(),
            desc: event.description.as_deref(),
            dur: event.estimated_duration_seconds,
            tss: event.estimated_training_stress_score,
            ifv: event.estimated_intensity_factor,
            np: event.estimated_normalized_power_watts,
        }
    }
}

#[derive(Serialize)]
struct CompactIntervalsStatus<'a> {
    a: &'a str,
    e: &'a str,
}

impl<'a> CompactIntervalsStatus<'a> {
    fn from_status(status: &'a IntervalsStatusContext) -> Self {
        Self {
            a: &status.activities,
            e: &status.events,
        }
    }
}

#[derive(Serialize)]
struct CompactProfile<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    fnm: Option<&'a str>,
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
    ap: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meds: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<&'a str>,
    acfg: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    av: Vec<CompactAvailabilityDay<'a>>,
}

impl<'a> CompactProfile<'a> {
    fn from_profile(profile: &'a AthleteProfileContext) -> Self {
        Self {
            fnm: profile.full_name.as_deref(),
            age: profile.age,
            hcm: profile.height_cm,
            wkg: profile.weight_kg,
            ftp: profile.ftp_watts,
            hrm: profile.hr_max_bpm,
            vo2: profile.vo2_max,
            ap: profile.athlete_prompt.as_deref(),
            meds: profile.medications.as_deref(),
            notes: profile.athlete_notes.as_deref(),
            acfg: profile.availability_configured,
            av: profile
                .weekly_availability
                .iter()
                .map(CompactAvailabilityDay::from_day)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CompactAvailabilityDay<'a> {
    wd: &'a str,
    a: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    mdm: Option<u16>,
}

impl<'a> CompactAvailabilityDay<'a> {
    fn from_day(
        day: &'a crate::domain::training_context::model::WeeklyAvailabilityContext,
    ) -> Self {
        Self {
            wd: day.weekday.as_str(),
            a: day.available,
            mdm: day.max_duration_minutes,
        }
    }
}

#[derive(Serialize)]
struct CompactHistory<'a> {
    ws: &'a str,
    we: &'a str,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lt: Vec<CompactHistoricalLoadTrend<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    w: Vec<CompactHistoricalWorkout<'a>>,
}

impl<'a> CompactHistory<'a> {
    fn from_history(history: &'a HistoricalTrainingContext) -> Self {
        Self {
            ws: &history.window_start,
            we: &history.window_end,
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
            lt: history
                .load_trend
                .iter()
                .map(CompactHistoricalLoadTrend::from_point)
                .collect(),
            w: history
                .workouts
                .iter()
                .map(CompactHistoricalWorkout::from_workout)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CompactHistoricalLoadTrend<'a> {
    d: &'a str,
    days: u8,
    tss: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    t7: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    t28: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ctl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tsb: Option<f64>,
}

impl<'a> CompactHistoricalLoadTrend<'a> {
    fn from_point(point: &'a HistoricalLoadTrendPoint) -> Self {
        Self {
            d: &point.date,
            days: point.sample_days,
            tss: point.period_tss,
            t7: point.rolling_tss_7d,
            t28: point.rolling_tss_28d,
            ctl: point.ctl,
            atl: point.atl,
            tsb: point.tsb,
        }
    }
}

#[derive(Serialize)]
struct CompactHistoricalWorkout<'a> {
    d: &'a str,
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ty: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dur: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tss: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ifv: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ef: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    np: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ftp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recap: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vi: Option<f64>,
    #[serde(skip_serializing_if = "crate::domain::training_context::packing::is_empty_slice")]
    pc: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bl: Vec<CompactPlannedWorkoutBlock>,
}

impl<'a> CompactHistoricalWorkout<'a> {
    fn from_workout(workout: &'a HistoricalWorkoutContext) -> Self {
        Self {
            d: &workout.date,
            id: &workout.activity_id,
            n: workout.name.as_deref(),
            ty: workout.activity_type.as_deref(),
            dur: workout.duration_seconds,
            tss: workout.training_stress_score,
            ifv: workout.intensity_factor,
            ef: workout.efficiency_factor,
            np: workout.normalized_power_watts,
            ftp: workout.ftp_watts,
            recap: workout.workout_recap.as_deref(),
            vi: workout.variability_index,
            pc: &workout.compressed_power_levels,
            bl: workout
                .interval_blocks
                .iter()
                .map(CompactPlannedWorkoutBlock::from_block)
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct CompactPlannedWorkoutBlock {
    dur: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    minp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maxp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minw: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maxw: Option<i32>,
}

impl CompactPlannedWorkoutBlock {
    pub(super) fn from_block(block: &PlannedWorkoutBlockContext) -> Self {
        Self {
            dur: block.duration_seconds,
            minp: block.min_percent_ftp,
            maxp: block.max_percent_ftp,
            minw: block.min_target_watts,
            maxw: block.max_target_watts,
        }
    }
}
