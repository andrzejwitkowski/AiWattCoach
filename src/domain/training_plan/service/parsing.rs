use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    ai_workflow::ValidationIssue,
    identity::Clock,
    intervals::{parse_planned_workout_days, PlannedWorkoutDay},
};

use super::{ParsedPlanWindow, TrainingPlanGenerationService};
use crate::domain::training_plan::{
    TrainingPlanDay, TrainingPlanError, TrainingPlanGenerationOperationRepository,
    TrainingPlanGenerator, TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
    TrainingPlanWorkoutSummaryPort,
};

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time>
    TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
{
    fn map_parsed_day(day: PlannedWorkoutDay) -> TrainingPlanDay {
        let date = day.date.clone();
        let rest_day = day.is_rest_day();
        let workout = day.into_workout();
        TrainingPlanDay {
            date,
            rest_day,
            workout,
        }
    }

    pub(super) fn split_into_day_blocks(
        &self,
        input: &str,
    ) -> Result<Vec<(String, String)>, TrainingPlanError> {
        let mut blocks = Vec::new();
        let mut current_date: Option<String> = None;
        let mut current_lines = Vec::new();

        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            if Self::is_exact_date(line) {
                if let Some(date) = current_date.take() {
                    let mut block = Vec::with_capacity(current_lines.len() + 1);
                    block.push(date.clone());
                    block.extend(current_lines.clone());
                    blocks.push((date, block.join("\n")));
                    current_lines.clear();
                }
                current_date = Some(line.to_string());
                continue;
            }

            if current_date.is_none() {
                return Err(TrainingPlanError::Validation(
                    "content before first date header".to_string(),
                ));
            }

            current_lines.push(line.to_string());
        }

        if let Some(date) = current_date {
            let mut block = Vec::with_capacity(current_lines.len() + 1);
            block.push(date.clone());
            block.extend(current_lines);
            blocks.push((date, block.join("\n")));
        }

        Ok(blocks)
    }

    pub(super) fn is_exact_date(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 10
            && bytes[0..4].iter().all(u8::is_ascii_digit)
            && bytes[4] == b'-'
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[7] == b'-'
            && bytes[8..10].iter().all(u8::is_ascii_digit)
    }

    pub(super) fn parse_window(
        &self,
        raw_window: &str,
    ) -> Result<ParsedPlanWindow, TrainingPlanError> {
        let blocks = self.split_into_day_blocks(raw_window)?;
        let mut days_by_date = BTreeMap::new();
        let mut issues = Vec::new();
        let mut invalid_day_sections = Vec::new();

        for (date, block) in blocks {
            if days_by_date.contains_key(&date) {
                return Err(TrainingPlanError::Validation(format!(
                    "duplicate planned workout day: {date}"
                )));
            }

            match parse_planned_workout_days(&block) {
                Ok(parsed) => {
                    if let Some(day) = parsed.days.into_iter().next() {
                        days_by_date.insert(date, Self::map_parsed_day(day));
                    }
                }
                Err(error) => {
                    issues.push(ValidationIssue {
                        scope: date,
                        message: error.to_string(),
                    });
                    invalid_day_sections.push(block);
                }
            }
        }

        Ok(ParsedPlanWindow {
            days_by_date,
            issues,
            invalid_day_sections,
        })
    }

    pub(super) fn merge_corrections(
        &self,
        base_days: &mut BTreeMap<String, TrainingPlanDay>,
        corrected_days: BTreeMap<String, TrainingPlanDay>,
        invalid_dates: &BTreeSet<String>,
    ) {
        for (date, day) in corrected_days {
            if invalid_dates.contains(&date) {
                base_days.insert(date, day);
            }
        }
    }
}
