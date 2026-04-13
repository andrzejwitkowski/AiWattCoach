use crate::domain::calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh};

use super::{BoxFuture, SpecialDay, SpecialDayError, SpecialDayRepository};

#[derive(Clone)]
pub struct SpecialDayService<Repository, Refresh = NoopCalendarEntryViewRefresh>
where
    Repository: SpecialDayRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    repository: Repository,
    refresh: Refresh,
}

impl<Repository> SpecialDayService<Repository>
where
    Repository: SpecialDayRepository + Clone + 'static,
{
    pub fn new(repository: Repository) -> Self {
        Self {
            repository,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<Repository, Refresh> SpecialDayService<Repository, Refresh>
where
    Repository: SpecialDayRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> SpecialDayService<Repository, NewRefresh>
    where
        NewRefresh: CalendarEntryViewRefreshPort + Clone + 'static,
    {
        SpecialDayService {
            repository: self.repository,
            refresh,
        }
    }

    pub fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> BoxFuture<Result<SpecialDay, SpecialDayError>> {
        let repository = self.repository.clone();
        let refresh = self.refresh.clone();
        Box::pin(async move {
            let stored = repository.upsert(special_day).await?;
            if let Err(error) = refresh
                .refresh_range_for_user(&stored.user_id, &stored.date, &stored.date)
                .await
            {
                tracing::warn!(
                    user_id = %stored.user_id,
                    date = %stored.date,
                    %error,
                    "special day upsert succeeded but calendar view refresh failed"
                );
            }
            Ok(stored)
        })
    }

    pub fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            repository
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
        })
    }
}
