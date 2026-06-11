use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    ai_workflow::ValidationIssue,
    calendar_view::CalendarEntryViewRefreshPort,
    identity::Clock,
    intervals::{ensure_planned_workout_title, parse_planned_workout_days, PlannedWorkoutDay},
};

use super::{ParsedPlanWindow, TrainingPlanGenerationService};
use crate::domain::training_plan::{
    TrainingPlanDay, TrainingPlanError, TrainingPlanGenerationOperationRepository,
    TrainingPlanGenerator, TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
    TrainingPlanWorkoutSummaryPort,
};

fn is_exact_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

pub(crate) fn split_into_day_blocks(
    input: &str,
) -> Result<Vec<(String, String)>, TrainingPlanError> {
    let mut blocks = Vec::new();
    let mut current_date: Option<String> = None;
    let mut current_lines = Vec::new();
    let mut saw_non_empty_line = false;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        saw_non_empty_line = true;

        if is_exact_date(line) {
            if let Some(date) = current_date.take() {
                let lines = std::mem::take(&mut current_lines);
                let mut block = Vec::with_capacity(lines.len() + 1);
                block.push(date.clone());
                block.extend(lines);
                blocks.push((date, block.join("\n")));
            }
            current_date = Some(line.to_string());
            continue;
        }

        if current_date.is_none() {
            continue;
        }

        current_lines.push(line.to_string());
    }

    if let Some(date) = current_date {
        let mut block = Vec::with_capacity(current_lines.len() + 1);
        block.push(date.clone());
        block.extend(current_lines);
        blocks.push((date, block.join("\n")));
    }

    if blocks.is_empty() && saw_non_empty_line {
        return Err(TrainingPlanError::Validation(
            "content before first date header".to_string(),
        ));
    }

    Ok(blocks)
}

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time, Refresh>
    TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
        Refresh,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    fn map_parsed_day(day: PlannedWorkoutDay) -> TrainingPlanDay {
        let date = day.date.clone();
        let rest_day = day.is_rest_day();
        let rest_day_reason = day.rest_day_reason().map(ToString::to_string);
        let workout = day.into_workout().map(ensure_planned_workout_title);
        TrainingPlanDay {
            date,
            rest_day,
            rest_day_reason,
            workout,
        }
    }

    pub(super) fn split_into_day_blocks(
        &self,
        input: &str,
    ) -> Result<Vec<(String, String)>, TrainingPlanError> {
        split_into_day_blocks(input)
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

#[cfg(test)]
mod tests {
    use super::split_into_day_blocks;
    use crate::domain::training_plan::TrainingPlanError;

    #[test]
    fn split_into_day_blocks_skips_preamble_before_first_date_header() {
        let blocks = split_into_day_blocks(
            "Symulacja potwierdza zdrowa progresje.\n\n2026-04-06\nRest Day\n\n2026-04-07\nEndurance\n- 45m 65%",
        )
        .expect("expected parsed day blocks");

        assert_eq!(
            blocks,
            vec![
                ("2026-04-06".to_string(), "2026-04-06\nRest Day".to_string()),
                (
                    "2026-04-07".to_string(),
                    "2026-04-07\nEndurance\n- 45m 65%".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn split_into_day_blocks_still_errors_when_no_date_header_exists() {
        let error = split_into_day_blocks("Symulacja potwierdza zdrowa progresje.")
            .expect_err("expected missing date header to fail");

        assert_eq!(
            error,
            TrainingPlanError::Validation("content before first date header".to_string())
        );
    }
}
