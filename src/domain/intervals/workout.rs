mod matching;
mod parser;
mod pest_parser;

use serde::{Deserialize, Serialize};

pub use matching::find_best_activity_match;
pub use parser::parse_workout_doc;
pub(crate) use pest_parser::parse_workout_ast;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParsedWorkoutDoc {
    pub intervals: Vec<WorkoutIntervalDefinition>,
    pub segments: Vec<WorkoutSegment>,
    pub summary: WorkoutSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutIntervalDefinition {
    pub definition: String,
    pub repeat_count: usize,
    pub duration_seconds: Option<i32>,
    pub target_percent_ftp: Option<f64>,
    pub min_target_percent_ftp: Option<f64>,
    pub max_target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSegment {
    pub order: usize,
    pub label: String,
    pub duration_seconds: i32,
    pub start_offset_seconds: i32,
    pub end_offset_seconds: i32,
    pub target_percent_ftp: Option<f64>,
    pub min_target_percent_ftp: Option<f64>,
    pub max_target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSummary {
    pub total_segments: usize,
    pub total_duration_seconds: i32,
    pub estimated_normalized_power_watts: Option<i32>,
    pub estimated_average_power_watts: Option<i32>,
    pub estimated_intensity_factor: Option<f64>,
    pub estimated_training_stress_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActualWorkoutMatch {
    pub activity_id: String,
    pub activity_name: Option<String>,
    pub start_date_local: String,
    pub power_values: Vec<i32>,
    pub cadence_values: Vec<i32>,
    pub heart_rate_values: Vec<i32>,
    pub speed_values: Vec<f64>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub training_stress_score: Option<i32>,
    pub intensity_factor: Option<f64>,
    pub compliance_score: f64,
    pub matched_intervals: Vec<MatchedWorkoutInterval>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchedWorkoutInterval {
    pub planned_segment_order: usize,
    pub planned_label: String,
    pub planned_duration_seconds: i32,
    pub target_percent_ftp: Option<f64>,
    pub min_target_percent_ftp: Option<f64>,
    pub max_target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
    pub actual_interval_id: Option<i32>,
    pub actual_start_time_seconds: Option<i32>,
    pub actual_end_time_seconds: Option<i32>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub average_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub compliance_score: f64,
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::pest_parser::{
        parse_workout_ast, CadenceRange, ParserTarget, StepAmount, StepKind, WorkoutItem,
    };

    #[test]
    fn pest_parser_parses_simple_ftp_workout() {
        let parsed = parse_workout_ast("- 10m 95%").expect("parse should succeed");

        assert_eq!(parsed.items.len(), 1);
    }

    #[test]
    fn pest_parser_parses_minute_suffix_variant() {
        let parsed = parse_workout_ast("- 5min 55%").expect("parse should succeed");

        let WorkoutItem::Step(step) = &parsed.items[0] else {
            panic!("expected step item");
        };

        assert_eq!(step.amount, StepAmount::DurationMinutes(5));
        assert_eq!(
            step.target,
            Some(ParserTarget::PercentFtp {
                min: 55.0,
                max: 55.0,
            })
        );
    }

    #[test]
    fn pest_parser_supports_hour_and_second_units() {
        let hour = parse_workout_ast("- 1h 95%").expect("hour parse should succeed");
        let seconds = parse_workout_ast("- 30s 55%").expect("seconds parse should succeed");

        let WorkoutItem::Step(hour_step) = &hour.items[0] else {
            panic!("expected hour step item");
        };
        let WorkoutItem::Step(seconds_step) = &seconds.items[0] else {
            panic!("expected seconds step item");
        };

        assert_eq!(hour_step.amount, StepAmount::DurationMinutes(60));
        assert_eq!(seconds_step.amount, StepAmount::DurationMinutes(1));
    }

    #[test]
    fn pest_parser_normalizes_arbitrary_second_granularity() {
        let parsed = parse_workout_ast("- 45s 55%").expect("parse should succeed");

        let WorkoutItem::Step(step) = &parsed.items[0] else {
            panic!("expected step item");
        };

        assert_eq!(step.amount, StepAmount::DurationMinutes(1));
    }

    #[test]
    fn pest_parser_supports_meter_and_mile_distances() {
        let meters = parse_workout_ast("- 400mtr 55%").expect("meters parse");
        let miles = parse_workout_ast("- 1mi 8:00/mi Pace").expect("miles parse");

        let WorkoutItem::Step(meter_step) = &meters.items[0] else {
            panic!("expected meter step item");
        };
        let WorkoutItem::Step(mile_step) = &miles.items[0] else {
            panic!("expected mile step item");
        };

        assert_eq!(meter_step.amount, StepAmount::DistanceKilometers(0.4));
        assert_eq!(mile_step.amount, StepAmount::DistanceKilometers(1.609_344));
    }

    #[test]
    fn pest_parser_rejects_non_positive_time_amounts() {
        let zero_minutes = parse_workout_ast("- 0m 55%").expect_err("0m should fail");
        let zero_hours = parse_workout_ast("- 0h 55%").expect_err("0h should fail");
        let zero_seconds = parse_workout_ast("- 0s 55%").expect_err("0s should fail");

        assert!(zero_minutes
            .to_string()
            .contains("time amount must be positive"));
        assert!(zero_hours
            .to_string()
            .contains("time amount must be positive"));
        assert!(zero_seconds
            .to_string()
            .contains("time amount must be positive"));
    }

    #[test]
    fn pest_parser_rejects_non_positive_distance_amounts() {
        let zero_km = parse_workout_ast("- 0km 55%").expect_err("0km should fail");
        let zero_meters = parse_workout_ast("- 0mtr 55%").expect_err("0mtr should fail");
        let zero_miles = parse_workout_ast("- 0mi 8:00/mi Pace").expect_err("0mi should fail");

        assert!(zero_km
            .to_string()
            .contains("distance amount must be positive"));
        assert!(zero_meters
            .to_string()
            .contains("distance amount must be positive"));
        assert!(zero_miles
            .to_string()
            .contains("distance amount must be positive"));
    }

    #[test]
    fn pest_parser_parses_repeat_block() {
        let parsed =
            parse_workout_ast("Main Set 4x\n- 2m 95%\n- 2m 55%").expect("parse should succeed");

        assert_eq!(parsed.items.len(), 1);
    }

    #[test]
    fn pest_parser_ends_repeat_block_at_blank_line() {
        let parsed = parse_workout_ast("Main Set 2x\n- 2m 95%\n- 2m 55%\n\n- 10m 60%")
            .expect("parse should succeed");

        assert_eq!(parsed.items.len(), 2);

        let WorkoutItem::RepeatBlock(repeat) = &parsed.items[0] else {
            panic!("expected repeat block item");
        };
        let WorkoutItem::Step(step) = &parsed.items[1] else {
            panic!("expected trailing step item");
        };

        assert_eq!(repeat.steps.len(), 2);
        assert_eq!(step.amount, StepAmount::DurationMinutes(10));
    }

    #[test]
    fn pest_parser_rejects_repeat_block_without_steps() {
        let error = parse_workout_ast("Main Set 4x").expect_err("parse should fail");

        assert!(error.to_string().contains("repeat block"));
    }

    #[test]
    fn pest_parser_rejects_malformed_input_without_panicking() {
        let error = parse_workout_ast("- 10m ???").expect_err("parse should fail");

        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn pest_parser_rejects_zero_repeat_count() {
        let error = parse_workout_ast("Main Set 0x\n- 2m 95%").expect_err("parse should fail");

        assert!(error.to_string().contains("repeat count"));
    }

    #[test]
    fn pest_parser_parses_pace_step() {
        let parsed = parse_workout_ast("- 5km 5:00/km Pace").expect("parse should succeed");

        let WorkoutItem::Step(step) = &parsed.items[0] else {
            panic!("expected step item");
        };

        assert_eq!(step.amount, StepAmount::DistanceKilometers(5.0));
        assert_eq!(
            step.target,
            Some(ParserTarget::Pace {
                value: "5:00/km".to_string()
            })
        );
    }

    #[test]
    fn pest_parser_parses_hr_and_lthr_targets() {
        let hr = parse_workout_ast("- 20m 75-80% Hr").expect("hr parse");
        let lthr = parse_workout_ast("- 20m 90-95% lThR").expect("lthr parse");

        let WorkoutItem::Step(hr_step) = &hr.items[0] else {
            panic!("expected hr step item");
        };
        let WorkoutItem::Step(lthr_step) = &lthr.items[0] else {
            panic!("expected lthr step item");
        };

        assert_eq!(
            hr_step.target,
            Some(ParserTarget::PercentHr {
                min: 75.0,
                max: 80.0,
            })
        );
        assert_eq!(
            lthr_step.target,
            Some(ParserTarget::PercentLthr {
                min: 90.0,
                max: 95.0,
            })
        );
    }

    #[test]
    fn pest_parser_parses_ramp_cadence_and_text_metadata() {
        let parsed = parse_workout_ast("- Warmup 10m ramp 50-70% 90RpM text=\"Relax shoulders\"")
            .expect("parse should succeed");

        let WorkoutItem::Step(step) = &parsed.items[0] else {
            panic!("expected step item");
        };

        assert_eq!(step.cue.as_deref(), Some("Warmup"));
        assert_eq!(step.kind, StepKind::Ramp);
        assert_eq!(
            step.target,
            Some(ParserTarget::PercentFtp {
                min: 50.0,
                max: 70.0,
            })
        );
        assert_eq!(
            step.cadence_rpm,
            Some(CadenceRange {
                min_rpm: 90,
                max_rpm: 90,
            })
        );
        assert_eq!(step.text.as_deref(), Some("Relax shoulders"));
    }

    #[test]
    fn pest_parser_parses_escaped_quotes_in_text_metadata() {
        let parsed = parse_workout_ast("- Warmup 10m text=\"Relax \\\"now\\\"\"")
            .expect("parse should succeed");

        let WorkoutItem::Step(step) = &parsed.items[0] else {
            panic!("expected step item");
        };

        assert_eq!(step.text.as_deref(), Some("Relax \"now\""));
    }

    #[test]
    fn pest_parser_rejects_multiline_text_metadata() {
        let error = parse_workout_ast("- Warmup 10m text=\"line one\nline two\"")
            .expect_err("parse should fail");

        assert!(!error.to_string().is_empty());
    }
}
