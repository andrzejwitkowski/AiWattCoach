use std::sync::{Arc, Mutex};

use super::{SpecialDay, SpecialDayKind, SpecialDayRepository, SpecialDayService};
use crate::domain::calendar_view::{
    CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort,
};

#[test]
fn special_day_uses_local_canonical_id_and_kind() {
    let day = SpecialDay::new(
        "special-1".to_string(),
        "user-1".to_string(),
        "2026-05-02".to_string(),
        SpecialDayKind::Illness,
        Some("Sick day".to_string()),
        Some("Fever and sore throat".to_string()),
    );

    assert_eq!(day.special_day_id, "special-1");
    assert_eq!(day.user_id, "user-1");
    assert_eq!(day.date, "2026-05-02");
    assert_eq!(day.kind, SpecialDayKind::Illness);
    assert_eq!(day.title.as_deref(), Some("Sick day"));
    assert_eq!(day.description.as_deref(), Some("Fever and sore throat"));
}

fn assert_special_day_repository<T: SpecialDayRepository>() {}

#[test]
fn special_day_repository_trait_is_usable() {
    assert_special_day_repository::<super::ports::NoopSpecialDayRepository>();
}

#[tokio::test]
async fn special_day_repository_lists_by_user_and_date_range() {
    let repository = super::ports::NoopSpecialDayRepository::default();
    repository
        .upsert(sample_day("special-2", "user-1", "2026-05-02"))
        .await
        .unwrap();
    repository
        .upsert(sample_day("special-1", "user-1", "2026-05-01"))
        .await
        .unwrap();
    repository
        .upsert(sample_day("special-3", "user-2", "2026-05-01"))
        .await
        .unwrap();

    let days = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(days.len(), 2);
    assert_eq!(days[0].special_day_id, "special-1");
    assert_eq!(days[1].special_day_id, "special-2");
}

#[tokio::test]
async fn special_day_service_refreshes_calendar_view_for_day_after_upsert() {
    let repository = super::ports::NoopSpecialDayRepository::default();
    let refresh = RecordingCalendarRefresh::default();
    let service =
        SpecialDayService::new(repository.clone()).with_calendar_view_refresh(refresh.clone());

    let stored = service
        .upsert(sample_day("special-1", "user-1", "2026-05-01"))
        .await
        .unwrap();

    assert_eq!(stored.special_day_id, "special-1");
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-01".to_string(),
            "2026-05-01".to_string()
        )]
    );
}

#[tokio::test]
async fn special_day_service_keeps_successful_upsert_when_refresh_fails() {
    let repository = super::ports::NoopSpecialDayRepository::default();
    let service = SpecialDayService::new(repository.clone())
        .with_calendar_view_refresh(FailingCalendarRefresh);

    let stored = service
        .upsert(sample_day("special-1", "user-1", "2026-05-01"))
        .await
        .unwrap();

    assert_eq!(stored.special_day_id, "special-1");
    let persisted = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-01")
        .await
        .unwrap();
    assert_eq!(persisted.len(), 1);
}

#[derive(Clone, Default)]
struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push((user_id, oldest, newest));
            Ok(Vec::new())
        })
    }
}

#[derive(Clone, Default)]
struct FailingCalendarRefresh;

impl CalendarEntryViewRefreshPort for FailingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        Box::pin(async {
            Err(CalendarEntryViewError::Repository(
                "refresh unavailable".to_string(),
            ))
        })
    }
}

fn sample_day(special_day_id: &str, user_id: &str, date: &str) -> SpecialDay {
    SpecialDay::new(
        special_day_id.to_string(),
        user_id.to_string(),
        date.to_string(),
        SpecialDayKind::Illness,
        Some("Illness".to_string()),
        Some("Recovery day".to_string()),
    )
}
