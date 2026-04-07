use std::collections::{BTreeMap, BTreeSet};

use crate::domain::ai_workflow::ValidationIssue;

pub(super) fn merge_unresolved_issues(
    previous: &[ValidationIssue],
    corrected: &[ValidationIssue],
    corrected_dates: &BTreeSet<String>,
    corrected_invalid_dates: &BTreeSet<String>,
) -> Vec<ValidationIssue> {
    let mut merged_by_scope = previous
        .iter()
        .filter(|issue| {
            !corrected_dates.contains(&issue.scope)
                || corrected_invalid_dates.contains(&issue.scope)
        })
        .map(|issue| (issue.scope.clone(), issue.clone()))
        .collect::<BTreeMap<_, _>>();

    for issue in corrected {
        merged_by_scope.insert(issue.scope.clone(), issue.clone());
    }

    merged_by_scope.into_values().collect()
}

pub(super) fn merge_invalid_day_sections(
    previous_sections: &[String],
    corrected_sections: &[String],
    corrected_dates: &BTreeSet<String>,
    corrected_invalid_dates: &BTreeSet<String>,
) -> Vec<String> {
    let mut merged_by_date = previous_sections
        .iter()
        .filter_map(|section| {
            section
                .lines()
                .next()
                .filter(|date| {
                    !corrected_dates.contains(*date) || corrected_invalid_dates.contains(*date)
                })
                .map(|date| (date.to_string(), section.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    for section in corrected_sections {
        if let Some(date) = section.lines().next() {
            merged_by_date.insert(date.to_string(), section.clone());
        }
    }

    merged_by_date.into_values().collect()
}
