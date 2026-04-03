use serde::Serialize;

use super::model::{
    AthleteProfileContext, HistoricalLoadTrendPoint, HistoricalTrainingContext,
    HistoricalWorkoutContext, IntervalsStatusContext, PlannedWorkoutBlockContext,
    PlannedWorkoutContext, PlannedWorkoutReference, RecentDayContext, RecentWorkoutContext,
    RenderedTrainingContext, SpecialDayContext, TrainingContext, UpcomingDayContext,
};

pub fn render_training_context(context: &TrainingContext) -> RenderedTrainingContext {
    let stable_payload = StablePayload::from_context(context);
    let volatile_payload = VolatilePayload::from_context(context);
    let stable_context =
        serde_json::to_string(&stable_payload).expect("stable training context should serialize");
    let volatile_context = serde_json::to_string(&volatile_payload)
        .expect("volatile training context should serialize");
    let approximate_tokens =
        approximate_token_count(&stable_context) + approximate_token_count(&volatile_context);

    RenderedTrainingContext {
        stable_context,
        volatile_context,
        approximate_tokens,
    }
}

pub fn approximate_token_count(value: &str) -> usize {
    value.chars().count().div_ceil(3)
}

fn is_empty_slice<T>(value: &[T]) -> bool {
    value.is_empty()
}

#[derive(Serialize)]
struct StablePayload<'a> {
    v: u8,
    i: CompactIntervalsStatus<'a>,
    p: CompactProfile<'a>,
    h: CompactHistory<'a>,
}

impl<'a> StablePayload<'a> {
    fn from_context(context: &'a TrainingContext) -> Self {
        Self {
            v: 1,
            i: CompactIntervalsStatus::from_status(&context.intervals_status),
            p: CompactProfile::from_profile(&context.profile),
            h: CompactHistory::from_history(&context.history),
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
struct VolatilePayload<'a> {
    v: u8,
    g: i64,
    fx: CompactFocus<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rd: Vec<CompactRecentDay<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ud: Vec<CompactUpcomingDay<'a>>,
}

impl<'a> VolatilePayload<'a> {
    fn from_context(context: &'a TrainingContext) -> Self {
        Self {
            v: 1,
            g: context.generated_at_epoch_seconds,
            fx: CompactFocus {
                id: &context.focus_workout_id,
                k: &context.focus_kind,
            },
            rd: context
                .recent_days
                .iter()
                .map(CompactRecentDay::from_recent_day)
                .collect(),
            ud: context
                .upcoming_days
                .iter()
                .map(CompactUpcomingDay::from_upcoming_day)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CompactFocus<'a> {
    id: &'a str,
    k: &'a str,
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
    vi: Option<f64>,
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
            vi: workout.variability_index,
        }
    }
}

#[derive(Serialize)]
struct CompactRecentDay<'a> {
    d: &'a str,
    fr: bool,
    sick: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    sickn: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    w: Vec<CompactRecentWorkout<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pw: Vec<CompactPlannedWorkout<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sd: Vec<CompactSpecialDay<'a>>,
}

impl<'a> CompactRecentDay<'a> {
    fn from_recent_day(day: &'a RecentDayContext) -> Self {
        Self {
            d: &day.date,
            fr: day.free_day,
            sick: day.sick_day,
            sickn: day.sick_note.as_deref(),
            w: day
                .workouts
                .iter()
                .map(CompactRecentWorkout::from_workout)
                .collect(),
            pw: day
                .planned_workouts
                .iter()
                .map(CompactPlannedWorkout::from_planned)
                .collect(),
            sd: day
                .special_days
                .iter()
                .map(CompactSpecialDay::from_special)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CompactRecentWorkout<'a> {
    id: &'a str,
    sd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ty: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tss: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ef: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ifv: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    np: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ftp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rpe: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vi: Option<f64>,
    #[serde(skip_serializing_if = "is_empty_slice")]
    p5: &'a [i32],
    #[serde(skip_serializing_if = "is_empty_slice")]
    c5: &'a [i32],
    #[serde(skip_serializing_if = "Option::is_none")]
    pw: Option<CompactPlannedWorkoutRef<'a>>,
}

impl<'a> CompactRecentWorkout<'a> {
    fn from_workout(workout: &'a RecentWorkoutContext) -> Self {
        Self {
            id: &workout.activity_id,
            sd: &workout.start_date_local,
            n: workout.name.as_deref(),
            ty: workout.activity_type.as_deref(),
            tss: workout.training_stress_score,
            ef: workout.efficiency_factor,
            ifv: workout.intensity_factor,
            np: workout.normalized_power_watts,
            ftp: workout.ftp_watts,
            rpe: workout.rpe,
            vi: workout.variability_index,
            p5: &workout.power_values_5s,
            c5: &workout.cadence_values_5s,
            pw: workout
                .planned_workout
                .as_ref()
                .map(CompactPlannedWorkoutRef::from_reference),
        }
    }
}

#[derive(Serialize)]
struct CompactPlannedWorkoutRef<'a> {
    id: i64,
    sd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    c: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bl: Vec<CompactPlannedWorkoutBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ifv: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    np: Option<i32>,
    done: bool,
}

impl<'a> CompactPlannedWorkoutRef<'a> {
    fn from_reference(reference: &'a PlannedWorkoutReference) -> Self {
        Self {
            id: reference.event_id,
            sd: &reference.start_date_local,
            n: reference.name.as_deref(),
            c: &reference.category,
            bl: reference
                .interval_blocks
                .iter()
                .map(CompactPlannedWorkoutBlock::from_block)
                .collect(),
            doc: reference.raw_workout_doc.as_deref(),
            tss: reference.estimated_training_stress_score,
            ifv: reference.estimated_intensity_factor,
            np: reference.estimated_normalized_power_watts,
            done: reference.completed,
        }
    }
}

#[derive(Serialize)]
struct CompactPlannedWorkout<'a> {
    id: i64,
    sd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    c: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bl: Vec<CompactPlannedWorkoutBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ifv: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    np: Option<i32>,
    done: bool,
}

impl<'a> CompactPlannedWorkout<'a> {
    fn from_planned(planned: &'a PlannedWorkoutContext) -> Self {
        Self {
            id: planned.event_id,
            sd: &planned.start_date_local,
            n: planned.name.as_deref(),
            c: &planned.category,
            bl: planned
                .interval_blocks
                .iter()
                .map(CompactPlannedWorkoutBlock::from_block)
                .collect(),
            doc: planned.raw_workout_doc.as_deref(),
            tss: planned.estimated_training_stress_score,
            ifv: planned.estimated_intensity_factor,
            np: planned.estimated_normalized_power_watts,
            done: planned.completed,
        }
    }
}

#[derive(Serialize)]
struct CompactPlannedWorkoutBlock {
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
    fn from_block(block: &PlannedWorkoutBlockContext) -> Self {
        Self {
            dur: block.duration_seconds,
            minp: block.min_percent_ftp,
            maxp: block.max_percent_ftp,
            minw: block.min_target_watts,
            maxw: block.max_target_watts,
        }
    }
}

#[derive(Serialize)]
struct CompactSpecialDay<'a> {
    id: i64,
    sd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    c: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    desc: Option<&'a str>,
}

impl<'a> CompactSpecialDay<'a> {
    fn from_special(special: &'a SpecialDayContext) -> Self {
        Self {
            id: special.event_id,
            sd: &special.start_date_local,
            n: special.name.as_deref(),
            c: &special.category,
            desc: special.description.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct CompactUpcomingDay<'a> {
    d: &'a str,
    fr: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pw: Vec<CompactPlannedWorkout<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sd: Vec<CompactSpecialDay<'a>>,
}

impl<'a> CompactUpcomingDay<'a> {
    fn from_upcoming_day(day: &'a UpcomingDayContext) -> Self {
        Self {
            d: &day.date,
            fr: day.free_day,
            pw: day
                .planned_workouts
                .iter()
                .map(CompactPlannedWorkout::from_planned)
                .collect(),
            sd: day
                .special_days
                .iter()
                .map(CompactSpecialDay::from_special)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::training_context::model::{
        HistoricalTrainingContext, PlannedWorkoutReference, RecentWorkoutContext,
    };

    #[test]
    fn compact_render_is_non_empty_and_estimates_tokens() {
        let context = TrainingContext {
            generated_at_epoch_seconds: 1,
            focus_workout_id: "workout-1".to_string(),
            focus_kind: "activity".to_string(),
            intervals_status: IntervalsStatusContext {
                activities: "ok".to_string(),
                events: "ok".to_string(),
            },
            profile: AthleteProfileContext {
                athlete_prompt: Some("Climb-focused athlete".to_string()),
                ..AthleteProfileContext::default()
            },
            history: HistoricalTrainingContext {
                window_start: "2025-10-01".to_string(),
                window_end: "2026-04-01".to_string(),
                load_trend: vec![HistoricalLoadTrendPoint {
                    date: "2026-03-31".to_string(),
                    sample_days: 1,
                    period_tss: 42,
                    rolling_tss_7d: Some(37.5),
                    rolling_tss_28d: Some(51.3),
                    ctl: Some(65.2),
                    atl: Some(58.6),
                    tsb: Some(6.6),
                }],
                ..HistoricalTrainingContext::default()
            },
            recent_days: vec![RecentDayContext {
                date: "2026-04-01".to_string(),
                sick_day: true,
                sick_note: Some("felt unwell".to_string()),
                workouts: vec![RecentWorkoutContext {
                    activity_id: "ride-1".to_string(),
                    start_date_local: "2026-04-01T08:00:00".to_string(),
                    power_values_5s: vec![200, 220],
                    cadence_values_5s: vec![85, 88],
                    planned_workout: Some(PlannedWorkoutReference {
                        event_id: 101,
                        start_date_local: "2026-04-01T07:00:00".to_string(),
                        category: "WORKOUT".to_string(),
                        interval_blocks: vec![PlannedWorkoutBlockContext {
                            duration_seconds: 480,
                            min_percent_ftp: Some(90.0),
                            max_percent_ftp: Some(95.0),
                            min_target_watts: Some(270),
                            max_target_watts: Some(285),
                        }],
                        completed: true,
                        ..PlannedWorkoutReference::default()
                    }),
                    ..RecentWorkoutContext::default()
                }],
                ..RecentDayContext::default()
            }],
            upcoming_days: Vec::new(),
        };

        let rendered = render_training_context(&context);

        assert!(rendered
            .stable_context
            .contains("\"ap\":\"Climb-focused athlete\""));
        assert!(rendered.stable_context.contains(
            "\"lt\":[{\"d\":\"2026-03-31\",\"days\":1,\"tss\":42,\"t7\":37.5,\"t28\":51.3"
        ));
        assert!(rendered.volatile_context.contains("\"sick\":true"));
        assert!(rendered
            .volatile_context
            .contains("\"sickn\":\"felt unwell\""));
        assert!(rendered.volatile_context.contains(
            "\"bl\":[{\"dur\":480,\"minp\":90.0,\"maxp\":95.0,\"minw\":270,\"maxw\":285}]"
        ));
        assert!(rendered.volatile_context.contains("\"p5\":[200,220]"));
        assert!(rendered.approximate_tokens > 0);
    }

    #[test]
    fn approximate_token_count_is_conservative() {
        assert_eq!(approximate_token_count("abcdef"), 2);
        assert_eq!(approximate_token_count("abcdefg"), 3);
    }

    #[test]
    fn compact_render_omits_nulls_and_empty_lists() {
        let rendered = render_training_context(&TrainingContext {
            generated_at_epoch_seconds: 1,
            focus_workout_id: "w1".to_string(),
            focus_kind: "summary".to_string(),
            intervals_status: IntervalsStatusContext {
                activities: "ok".to_string(),
                events: "ok".to_string(),
            },
            profile: AthleteProfileContext::default(),
            history: HistoricalTrainingContext::default(),
            recent_days: Vec::new(),
            upcoming_days: Vec::new(),
        });

        assert!(!rendered.stable_context.contains(":null"));
        assert!(!rendered.stable_context.contains("\"lt\":[]"));
        assert!(!rendered.volatile_context.contains("\"rd\":[]"));
        assert!(!rendered.volatile_context.contains("\"ud\":[]"));
    }
}
