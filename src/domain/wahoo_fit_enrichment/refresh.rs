use crate::domain::calendar_view::CalendarEntryViewRefreshPort;

use super::WahooFitEnrichmentError;

pub async fn refresh_completed_workout_day<Refresh>(
    refresh: &Refresh,
    user_id: &str,
    start_date_local: &str,
) -> Result<(), WahooFitEnrichmentError>
where
    Refresh: CalendarEntryViewRefreshPort,
{
    let day = start_date_local.get(..10).unwrap_or(start_date_local);
    refresh
        .refresh_range_for_user(user_id, day, day)
        .await
        .map(|_| ())
        .map_err(Into::into)
}
