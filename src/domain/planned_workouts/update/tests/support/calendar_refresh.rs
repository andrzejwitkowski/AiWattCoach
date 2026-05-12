use std::sync::{Arc, Mutex};

use crate::domain::calendar_view::{
    BoxFuture, CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort,
};

#[derive(Clone, Default)]
pub struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    pub fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().expect("refresh mutex poisoned").clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls
                .lock()
                .expect("refresh mutex poisoned")
                .push((user_id, oldest, newest));
            Ok(Vec::new())
        })
    }
}
