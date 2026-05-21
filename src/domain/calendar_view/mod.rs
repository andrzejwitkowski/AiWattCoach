mod integrity;
mod manual_refresh;
mod model;
mod planned_candidates;
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
pub use manual_refresh::{
    ManualCalendarRefreshResult, ManualCalendarRefreshService, ManualCalendarRefreshUseCases,
};
pub use model::{
    CalendarEntryKind, CalendarEntryRace, CalendarEntrySummary, CalendarEntrySync,
    CalendarEntryView, CalendarEntryViewError,
};
pub use planned_candidates::{
    select_visible_planned_workout_candidates,
    select_visible_planned_workout_candidates_with_sync_states, CalendarPlannedSyncKey,
    CalendarPlannedWorkoutCandidate, CalendarPlannedWorkoutOrigin, CalendarPlannedWorkoutSource,
};
pub use ports::{BoxFuture, CalendarEntryViewRepository};
pub use projection::{
    project_completed_workout_entry, project_planned_workout_entry,
    project_planned_workout_entry_with_supervisor, project_race_entry, project_special_day_entry,
};
pub use rebuild::{merge_workout_entries, rebuild_calendar_entries};
pub use refresh::{
    CalendarEntryViewRefreshPort, CalendarEntryViewRefreshService, NoopCalendarEntryViewRefresh,
};
pub use service::CalendarEntryViewService;
