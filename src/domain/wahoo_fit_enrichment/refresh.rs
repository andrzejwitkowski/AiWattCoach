use crate::domain::calendar_view::CalendarEntryViewRefreshPort;
use tracing::warn;

use super::WahooFitEnrichmentError;

pub async fn refresh_completed_workout_day<Refresh>(
    refresh: &Refresh,
    user_id: &str,
    start_date_local: &str,
) -> Result<(), WahooFitEnrichmentError>
where
    Refresh: CalendarEntryViewRefreshPort,
{
    let Some(day) = start_date_local.get(..10) else {
        warn!(
            user_id,
            start_date_local, "skipping calendar refresh for malformed workout date"
        );
        return Ok(());
    };
    refresh
        .refresh_range_for_user(user_id, day, day)
        .await
        .map(|_| ())
        .map_err(Into::into)
}
