#[derive(Clone, Debug, PartialEq)]
pub struct PlannedWorkoutContent {
    pub lines: Vec<PlannedWorkoutLine>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlannedWorkoutLine {
    BlankLine,
    Text(PlannedWorkoutText),
    Repeat(PlannedWorkoutRepeat),
    Step(PlannedWorkoutStep),
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
pub struct PlannedWorkout {
    pub planned_workout_id: String,
    pub user_id: String,
    pub date: String,
    pub rest_day: bool,
    pub rest_day_reason: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub event_type: Option<String>,
    pub workout: PlannedWorkoutContent,
    // None = older than any Some (legacy / projected).
    pub updated_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedWorkoutError {
    Repository(String),
}

impl std::fmt::Display for PlannedWorkoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PlannedWorkoutError {}

impl PlannedWorkout {
    pub fn new(
        planned_workout_id: String,
        user_id: String,
        date: String,
        workout: PlannedWorkoutContent,
    ) -> Self {
        Self {
            planned_workout_id,
            user_id,
            date,
            rest_day: false,
            rest_day_reason: None,
            name: None,
            description: None,
            event_type: None,
            workout,
            updated_at_epoch_seconds: None,
        }
    }

    pub fn with_event_metadata(
        mut self,
        name: Option<String>,
        description: Option<String>,
        event_type: Option<String>,
    ) -> Self {
        self.name = name;
        self.description = description;
        self.event_type = event_type;
        self
    }

    pub fn with_updated_at(mut self, updated_at_epoch_seconds: Option<i64>) -> Self {
        self.updated_at_epoch_seconds = updated_at_epoch_seconds;
        self
    }

    pub fn as_rest_day(mut self, reason: Option<String>) -> Self {
        self.rest_day = true;
        self.rest_day_reason = reason;
        self
    }
}

pub fn is_imported_row_removed_for_user_date(
    user_id: &str,
    date: &str,
    keep_planned_workout_ids: &[String],
    row_user_id: &str,
    row_date: &str,
    row_planned_workout_id: &str,
) -> bool {
    row_user_id == user_id
        && row_date == date
        && !keep_planned_workout_ids
            .iter()
            .any(|keep_id| keep_id == row_planned_workout_id)
}

pub fn delete_imported_planned_workouts_in_memory(
    workouts: &mut Vec<PlannedWorkout>,
    user_id: &str,
    date: &str,
    keep_planned_workout_ids: &[String],
) -> u64 {
    let before = workouts.len();
    workouts.retain(|workout| {
        !is_imported_row_removed_for_user_date(
            user_id,
            date,
            keep_planned_workout_ids,
            &workout.user_id,
            &workout.date,
            &workout.planned_workout_id,
        )
    });
    (before - workouts.len()) as u64
}
