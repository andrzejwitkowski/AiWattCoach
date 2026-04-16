#[derive(Clone, Debug, PartialEq)]
pub struct PlannedWorkoutContent {
    pub lines: Vec<PlannedWorkoutLine>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlannedWorkoutLine {
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
    pub name: Option<String>,
    pub description: Option<String>,
    pub event_type: Option<String>,
    pub workout: PlannedWorkoutContent,
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
            name: None,
            description: None,
            event_type: None,
            workout,
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
}
