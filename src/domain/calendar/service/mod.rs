mod errors;
mod list_events;
mod projected;
mod sync;

use crate::domain::{
    calendar_view::{
        CalendarEntryViewRefreshPort, CalendarEntryViewRepository, NoopCalendarEntryViewRefresh,
    },
    completed_workouts::CompletedWorkoutRepository,
    external_sync::{NoopProviderPollStateRepository, ProviderPollStateRepository},
    identity::Clock,
    intervals::IntervalsUseCases,
    planned_workout_tokens::{NoopPlannedWorkoutTokenRepository, PlannedWorkoutTokenRepository},
    training_plan::TrainingPlanProjectionRepository,
};

use super::{
    BoxFuture, CalendarError, CalendarEvent, CalendarUseCases, PlannedWorkoutSyncRepository,
    SyncPlannedWorkout,
};

#[derive(Clone)]
pub struct CalendarService<
    Intervals,
    Entries,
    Projections,
    Syncs,
    Time,
    Tokens = NoopPlannedWorkoutTokenRepository,
    PollStates = NoopProviderPollStateRepository,
    Refresh = NoopCalendarEntryViewRefresh,
    Completed = (),
> where
    Intervals: IntervalsUseCases + Clone + 'static,
    Entries: CalendarEntryViewRepository + Clone + 'static,
    Completed: CompletedWorkoutRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Syncs: PlannedWorkoutSyncRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Tokens: PlannedWorkoutTokenRepository + Clone + 'static,
    PollStates: ProviderPollStateRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    intervals: Intervals,
    entries: Entries,
    completed_workouts: Completed,
    projections: Projections,
    syncs: Syncs,
    clock: Time,
    planned_workout_tokens: Tokens,
    poll_states: PollStates,
    refresh: Refresh,
}

impl<Intervals, Entries, Projections, Syncs, Time>
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        NoopPlannedWorkoutTokenRepository,
        NoopProviderPollStateRepository,
        NoopCalendarEntryViewRefresh,
        (),
    >
where
    Intervals: IntervalsUseCases + Clone,
    Entries: CalendarEntryViewRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Time: Clock + Clone,
{
    pub fn new(
        intervals: Intervals,
        entries: Entries,
        projections: Projections,
        syncs: Syncs,
        clock: Time,
    ) -> Self {
        Self {
            intervals,
            entries,
            completed_workouts: (),
            projections,
            syncs,
            clock,
            planned_workout_tokens: NoopPlannedWorkoutTokenRepository::default(),
            poll_states: NoopProviderPollStateRepository,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<Intervals, Entries, Projections, Syncs, Time, Tokens, PollStates, Refresh, Completed>
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        PollStates,
        Refresh,
        Completed,
    >
where
    Intervals: IntervalsUseCases + Clone,
    Entries: CalendarEntryViewRepository + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Time: Clock + Clone,
    Tokens: PlannedWorkoutTokenRepository + Clone,
    PollStates: ProviderPollStateRepository + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    pub fn with_provider_poll_states<NewPollStates>(
        self,
        poll_states: NewPollStates,
    ) -> CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        NewPollStates,
        Refresh,
        Completed,
    >
    where
        NewPollStates: ProviderPollStateRepository + Clone,
    {
        CalendarService {
            intervals: self.intervals,
            entries: self.entries,
            completed_workouts: self.completed_workouts,
            projections: self.projections,
            syncs: self.syncs,
            clock: self.clock,
            planned_workout_tokens: self.planned_workout_tokens,
            poll_states,
            refresh: self.refresh,
        }
    }

    pub fn with_planned_workout_tokens<NewTokens>(
        self,
        planned_workout_tokens: NewTokens,
    ) -> CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        NewTokens,
        PollStates,
        Refresh,
        Completed,
    >
    where
        NewTokens: PlannedWorkoutTokenRepository + Clone,
    {
        CalendarService {
            intervals: self.intervals,
            entries: self.entries,
            completed_workouts: self.completed_workouts,
            projections: self.projections,
            syncs: self.syncs,
            clock: self.clock,
            planned_workout_tokens,
            poll_states: self.poll_states,
            refresh: self.refresh,
        }
    }

    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        PollStates,
        NewRefresh,
        Completed,
    >
    where
        NewRefresh: CalendarEntryViewRefreshPort + Clone,
    {
        CalendarService {
            intervals: self.intervals,
            entries: self.entries,
            completed_workouts: self.completed_workouts,
            projections: self.projections,
            syncs: self.syncs,
            clock: self.clock,
            planned_workout_tokens: self.planned_workout_tokens,
            poll_states: self.poll_states,
            refresh,
        }
    }

    pub fn with_completed_workouts<NewCompleted>(
        self,
        completed_workouts: NewCompleted,
    ) -> CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        PollStates,
        Refresh,
        NewCompleted,
    >
    where
        NewCompleted: CompletedWorkoutRepository + Clone,
    {
        CalendarService {
            intervals: self.intervals,
            entries: self.entries,
            completed_workouts,
            projections: self.projections,
            syncs: self.syncs,
            clock: self.clock,
            planned_workout_tokens: self.planned_workout_tokens,
            poll_states: self.poll_states,
            refresh: self.refresh,
        }
    }
}

impl<Intervals, Entries, Projections, Syncs, Time, Tokens, PollStates, Refresh, Completed>
    CalendarUseCases
    for CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        PollStates,
        Refresh,
        Completed,
    >
where
    Intervals: IntervalsUseCases + Clone,
    Entries: CalendarEntryViewRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Time: Clock + Clone,
    Tokens: PlannedWorkoutTokenRepository + Clone,
    PollStates: ProviderPollStateRepository + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
    Completed: CompletedWorkoutRepository + Clone,
{
    fn list_events(
        &self,
        user_id: &str,
        range: &crate::domain::intervals::DateRange,
    ) -> BoxFuture<Result<Vec<CalendarEvent>, CalendarError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_events_impl(&user_id, &range).await })
    }

    fn sync_planned_workout(
        &self,
        user_id: &str,
        request: SyncPlannedWorkout,
    ) -> BoxFuture<Result<CalendarEvent, CalendarError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.sync_planned_workout_impl(&user_id, request).await })
    }
}
