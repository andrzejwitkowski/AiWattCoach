mod parser;

use std::{error::Error, fmt};

pub use parser::{parse_planned_workout, parse_planned_workout_days, serialize_planned_workout};

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedWorkout {
    pub lines: Vec<PlannedWorkoutLine>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlannedWorkoutLine {
    Text(PlannedWorkoutText),
    Repeat(PlannedWorkoutRepeat),
    Step(PlannedWorkoutStep),
}

impl PlannedWorkoutLine {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.text.as_str()),
            _ => None,
        }
    }

    pub fn repeat(&self) -> Option<&PlannedWorkoutRepeat> {
        match self {
            Self::Repeat(repeat) => Some(repeat),
            _ => None,
        }
    }

    pub fn step(&self) -> Option<&PlannedWorkoutStep> {
        match self {
            Self::Step(step) => Some(step),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedWorkoutText {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedWorkoutRepeat {
    pub title: Option<String>,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedWorkoutStep {
    pub duration_seconds: i32,
    pub kind: PlannedWorkoutStepKind,
    pub target: PlannedWorkoutTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedWorkoutStepKind {
    Steady,
    Ramp,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlannedWorkoutTarget {
    PercentFtp { min: f64, max: f64 },
    WattsRange { min: i32, max: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedWorkoutDays {
    pub days: Vec<PlannedWorkoutDay>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedWorkoutDay {
    pub date: String,
    pub rest_day: bool,
    pub workout: Option<PlannedWorkout>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedWorkoutParseError {
    message: String,
}

impl PlannedWorkoutParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PlannedWorkoutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for PlannedWorkoutParseError {}
