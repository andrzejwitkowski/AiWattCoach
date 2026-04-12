#[derive(Clone, Debug, PartialEq)]
pub struct WorkoutAst {
    pub items: Vec<WorkoutItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StepAmount {
    DurationMinutes(i32),
    DistanceKilometers(f64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepKind {
    Steady,
    Ramp,
    FreeRide,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CadenceRange {
    pub min_rpm: i32,
    pub max_rpm: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParserTarget {
    PercentFtp { min: f64, max: f64 },
    PercentHr { min: f64, max: f64 },
    PercentLthr { min: f64, max: f64 },
    Pace { value: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkoutItem {
    Step(WorkoutStepAst),
    RepeatBlock(RepeatBlockAst),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepeatBlockAst {
    pub title: Option<String>,
    pub count: usize,
    pub steps: Vec<WorkoutStepAst>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkoutStepAst {
    pub cue: Option<String>,
    pub amount: StepAmount,
    pub kind: StepKind,
    pub target: Option<ParserTarget>,
    pub cadence_rpm: Option<CadenceRange>,
    pub text: Option<String>,
    pub raw: String,
}
