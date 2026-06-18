use chrono::NaiveDate;

use super::{
    model::{
        expand_inclusive_date_range, validate_past_date_changes_allowed,
        validate_write_range_ends_on_or_after, CreatePlannedRestDay, PlannedRestDay,
        PlannedRestDayError, UpdatePlannedRestDay,
    },
    service::PlannedRestDayService,
    PlannedRestDayRepository, PlannedRestDayUseCases,
};
use crate::domain::{
    identity::{Clock, IdGenerator},
    intervals::DateRange,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[test]
fn new_rejects_end_before_start() {
    let err = PlannedRestDay::new(
        "prd:u:1".into(),
        "user-1".into(),
        "2026-07-10".into(),
        "2026-07-09".into(),
        None,
        None,
        1,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}

#[test]
fn single_day_allowed_when_start_equals_end() {
    let day = PlannedRestDay::new(
        "prd:u:1".into(),
        "user-1".into(),
        "2026-07-10".into(),
        "2026-07-10".into(),
        Some("Recovery".into()),
        None,
        1,
        1,
    )
    .unwrap();
    assert_eq!(day.start_date, day.end_date);
}

#[test]
fn expand_inclusive_date_range_returns_each_day() {
    let dates = expand_inclusive_date_range("2026-07-01", "2026-07-03").unwrap();
    assert_eq!(
        dates,
        vec![
            "2026-07-01".to_string(),
            "2026-07-02".to_string(),
            "2026-07-03".to_string(),
        ]
    );
}

#[test]
fn expand_inclusive_date_range_rejects_invalid_range() {
    let err = expand_inclusive_date_range("2026-07-10", "2026-07-09").unwrap_err();
    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}

#[test]
fn validate_write_range_rejects_fully_past_end() {
    let err = validate_write_range_ends_on_or_after(
        NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        "2026-06-05",
    )
    .unwrap_err();
    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}

#[test]
fn validate_past_date_changes_rejects_date_edits() {
    let existing = PlannedRestDay::new(
        "prd:1".into(),
        "user-1".into(),
        "2026-05-01".into(),
        "2026-05-03".into(),
        None,
        None,
        1,
        1,
    )
    .unwrap();
    let request = UpdatePlannedRestDay {
        start_date: "2026-05-02".into(),
        end_date: "2026-05-03".into(),
        title: None,
        note: None,
    };

    let err = validate_past_date_changes_allowed(
        &existing,
        &request,
        NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}

#[derive(Clone)]
struct TestClock {
    today: NaiveDate,
}

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.today
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp()
    }
}

#[derive(Clone, Default)]
struct TestIdGenerator {
    counter: Arc<AtomicUsize>,
}

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, _prefix: &str) -> String {
        format!("prd-{}", self.counter.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Default)]
struct InMemoryPlannedRestDayRepository {
    entries: Arc<Mutex<Vec<PlannedRestDay>>>,
}

impl PlannedRestDayRepository for InMemoryPlannedRestDayRepository {
    fn list_intersecting_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> super::BoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>> {
        let entries = self.entries.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            Ok(entries
                .into_iter()
                .filter(|entry| {
                    entry.user_id == user_id
                        && entry.start_date <= range.newest
                        && entry.end_date >= range.oldest
                })
                .collect())
        })
    }

    fn find_by_id(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> super::BoxFuture<Result<Option<PlannedRestDay>, PlannedRestDayError>> {
        let entries = self.entries.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move {
            Ok(entries.into_iter().find(|entry| {
                entry.user_id == user_id && entry.planned_rest_day_id == planned_rest_day_id
            }))
        })
    }

    fn upsert(
        &self,
        entry: PlannedRestDay,
    ) -> super::BoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(index) = entries
            .iter()
            .position(|stored| stored.planned_rest_day_id == entry.planned_rest_day_id)
        {
            entries[index] = entry.clone();
        } else {
            entries.push(entry.clone());
        }
        Box::pin(async move { Ok(entry) })
    }

    fn delete(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> super::BoxFuture<Result<(), PlannedRestDayError>> {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|entry| {
            !(entry.user_id == user_id && entry.planned_rest_day_id == planned_rest_day_id)
        });
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move {
            let _ = (user_id, planned_rest_day_id);
            Ok(())
        })
    }
}

#[tokio::test]
async fn create_rejects_fully_past_range() {
    let service = PlannedRestDayService::new(
        InMemoryPlannedRestDayRepository::default(),
        TestClock {
            today: NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        },
        TestIdGenerator::default(),
    );

    let err = service
        .create(
            "user-1",
            CreatePlannedRestDay {
                start_date: "2026-06-01".into(),
                end_date: "2026-06-05".into(),
                title: None,
                note: None,
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}
