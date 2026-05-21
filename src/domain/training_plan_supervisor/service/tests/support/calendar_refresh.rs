use std::sync::{Arc, Mutex};

use crate::domain::calendar_view::{
    CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecordedCalendarRefresh {
    pub(crate) user_id: String,
    pub(crate) oldest: String,
    pub(crate) newest: String,
}

#[derive(Clone, Default)]
pub(crate) struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<RecordedCalendarRefresh>>>,
}

impl RecordingCalendarRefresh {
    pub(crate) fn calls(&self) -> Vec<RecordedCalendarRefresh> {
        self.calls
            .lock()
            .expect("calendar refresh mutex poisoned")
            .clone()
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
            calls
                .lock()
                .expect("calendar refresh mutex poisoned")
                .push(RecordedCalendarRefresh {
                    user_id,
                    oldest,
                    newest,
                });
            Ok(Vec::new())
        })
    }
}
