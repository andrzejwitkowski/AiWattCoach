use std::collections::HashMap;

use super::{CalendarEntryKind, CalendarEntryView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEntryIntegrityIssue {
    MissingEntry {
        entry_id: String,
    },
    DuplicateEntry {
        entry_id: String,
        count: usize,
    },
    TypeMismatch {
        entry_id: String,
        expected_kind: CalendarEntryKind,
        actual_kind: CalendarEntryKind,
    },
    OrphanEntry {
        entry_id: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CalendarEntryIntegrityReport {
    pub issues: Vec<CalendarEntryIntegrityIssue>,
}

impl CalendarEntryIntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn verify_calendar_entry_integrity(
    expected: &[CalendarEntryView],
    actual: &[CalendarEntryView],
) -> CalendarEntryIntegrityReport {
    let expected_by_id = expected
        .iter()
        .map(|entry| (entry.entry_id.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let mut counts_by_id = HashMap::<&str, usize>::new();

    for entry in actual {
        *counts_by_id.entry(entry.entry_id.as_str()).or_default() += 1;
    }

    let mut issues = counts_by_id
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(
            |(entry_id, count)| CalendarEntryIntegrityIssue::DuplicateEntry {
                entry_id: (*entry_id).to_string(),
                count: *count,
            },
        )
        .collect::<Vec<_>>();

    for expected_entry in expected {
        match actual
            .iter()
            .find(|entry| entry.entry_id == expected_entry.entry_id)
        {
            None => issues.push(CalendarEntryIntegrityIssue::MissingEntry {
                entry_id: expected_entry.entry_id.clone(),
            }),
            Some(actual_entry) if actual_entry.entry_kind != expected_entry.entry_kind => {
                issues.push(CalendarEntryIntegrityIssue::MissingEntry {
                    entry_id: expected_entry.entry_id.clone(),
                });
                issues.push(CalendarEntryIntegrityIssue::TypeMismatch {
                    entry_id: expected_entry.entry_id.clone(),
                    expected_kind: expected_entry.entry_kind.clone(),
                    actual_kind: actual_entry.entry_kind.clone(),
                });
            }
            Some(_) => {}
        }
    }

    for actual_entry in actual {
        if !expected_by_id.contains_key(actual_entry.entry_id.as_str()) {
            issues.push(CalendarEntryIntegrityIssue::OrphanEntry {
                entry_id: actual_entry.entry_id.clone(),
            });
        }
    }

    issues.sort_by(|left, right| issue_sort_key(left).cmp(&issue_sort_key(right)));

    CalendarEntryIntegrityReport { issues }
}

fn issue_sort_key(issue: &CalendarEntryIntegrityIssue) -> (u8, &str) {
    match issue {
        CalendarEntryIntegrityIssue::DuplicateEntry { entry_id, .. } => (0, entry_id),
        CalendarEntryIntegrityIssue::MissingEntry { entry_id } => (1, entry_id),
        CalendarEntryIntegrityIssue::TypeMismatch { entry_id, .. } => (2, entry_id),
        CalendarEntryIntegrityIssue::OrphanEntry { entry_id } => (3, entry_id),
    }
}
