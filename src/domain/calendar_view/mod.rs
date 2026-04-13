mod integrity;
mod model;
mod ports;
mod projection;
mod rebuild;
mod refresh;
mod service;
#[cfg(test)]
mod tests;

pub use integrity::{
    verify_calendar_entry_integrity, CalendarEntryIntegrityIssue, CalendarEntryIntegrityReport,
};
pub use model::{
    CalendarEntryKind, CalendarEntrySummary, CalendarEntrySync, CalendarEntryView,
    CalendarEntryViewError,
};
pub use ports::{BoxFuture, CalendarEntryViewRepository};
pub use projection::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry,
};
pub use rebuild::rebuild_calendar_entries;
pub use refresh::{
    CalendarEntryViewRefreshPort, CalendarEntryViewRefreshService, NoopCalendarEntryViewRefresh,
};
pub use service::CalendarEntryViewService;
