use serde::Serialize;

use super::super::is_empty_slice;
use super::stable::CompactPlannedWorkoutBlock;
use crate::domain::training_context::model::{
    PlannedWorkoutContext, PlannedWorkoutReference, ProjectedDayContext, ProjectedWorkoutContext,
    RecentDayContext, RecentWorkoutContext, SpecialDayContext, TrainingContext, UpcomingDayContext,
};

#[derive(Serialize)]
pub(crate) struct VolatilePayload<'a> {
    v: u8,
    g: i64,
    fx: CompactFocus<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rd: Vec<CompactRecentDay<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ud: Vec<CompactUpcomingDay<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pd: Vec<CompactProjectedDay<'a>>,
}

impl<'a> VolatilePayload<'a> {
    pub(crate) fn from_context(context: &'a TrainingContext) -> Self {
        Self {
            v: 1,
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
        }
    }
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
    recap: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vi: Option<f64>,
    #[serde(skip_serializing_if = "is_empty_slice")]
    pc: &'a [String],
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
            recap: workout.workout_recap.as_deref(),
            vi: workout.variability_index,
            pc: &workout.compressed_power_levels,
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

#[derive(Serialize)]
struct CompactProjectedDay<'a> {
    d: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    w: Vec<CompactProjectedWorkout<'a>>,
}

impl<'a> CompactProjectedDay<'a> {
    fn from_projected_day(day: &'a ProjectedDayContext) -> Self {
        Self {
            d: &day.date,
            w: day
                .workouts
                .iter()
                .map(CompactProjectedWorkout::from_projected_workout)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CompactProjectedWorkout<'a> {
    swid: &'a str,
    sd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bl: Vec<CompactPlannedWorkoutBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<&'a str>,
    rest: bool,
}

impl<'a> CompactProjectedWorkout<'a> {
    fn from_projected_workout(workout: &'a ProjectedWorkoutContext) -> Self {
        Self {
            swid: &workout.source_workout_id,
            sd: &workout.start_date_local,
            n: workout.name.as_deref(),
            bl: workout
                .interval_blocks
                .iter()
                .map(CompactPlannedWorkoutBlock::from_block)
                .collect(),
            doc: workout.raw_workout_doc.as_deref(),
            rest: workout.rest_day,
        }
    }
}
