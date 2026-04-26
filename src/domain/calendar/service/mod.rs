mod errors;
mod list_events;
mod projected;
mod sync;

use crate::domain::{
    calendar_view::{
        CalendarEntryViewRefreshPort, CalendarEntryViewRepository, NoopCalendarEntryViewRefresh,
    },
    completed_workouts::CompletedWorkoutRepository,
    identity::Clock,
    intervals::IntervalsUseCases,
    planned_workout_tokens::{NoopPlannedWorkoutTokenRepository, PlannedWorkoutTokenRepository},
    planned_workout_wahoo_syncs::{
        NoopPlannedWorkoutWahooSyncRepository, PlannedWorkoutWahooSyncRepository,
    },
    settings::{NoopUserSettingsRepository, UserSettingsRepository},
    training_plan::TrainingPlanProjectionRepository,
    wahoo::WahooUseCases,
};

use super::{
    BoxFuture, CalendarError, CalendarEvent, CalendarUseCases, PlannedWorkoutSyncRepository,
    SyncPlannedWorkout,
};

#[derive(Clone, Default)]
pub struct NoopWahooUseCases;

#[derive(Clone)]
pub struct CalendarService<
    Intervals,
    Entries,
    Projections,
    Syncs,
    Time,
    Wahoo = NoopWahooUseCases,
    WahooSyncs = NoopPlannedWorkoutWahooSyncRepository,
    Settings = NoopUserSettingsRepository,
    Tokens = NoopPlannedWorkoutTokenRepository,
    Refresh = NoopCalendarEntryViewRefresh,
    Completed = (),
> where
    Intervals: IntervalsUseCases + Clone + 'static,
    Entries: CalendarEntryViewRepository + Clone + 'static,
    Completed: CompletedWorkoutRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Syncs: PlannedWorkoutSyncRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Wahoo: WahooUseCases + Clone + 'static,
    WahooSyncs: PlannedWorkoutWahooSyncRepository + Clone + 'static,
    Settings: UserSettingsRepository + Clone + 'static,
    Tokens: PlannedWorkoutTokenRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    intervals: Intervals,
    entries: Entries,
    completed_workouts: Completed,
    projections: Projections,
    syncs: Syncs,
    clock: Time,
    wahoo: Wahoo,
    wahoo_syncs: WahooSyncs,
    settings: Settings,
    planned_workout_tokens: Tokens,
    refresh: Refresh,
}

impl crate::domain::wahoo::WahooUseCases for NoopWahooUseCases {
    fn begin_connect(
        &self,
        _user_id: &str,
        _return_to: Option<String>,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooAuthStart, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn finish_connect(
        &self,
        _user_id: &str,
        _state: &str,
        _code: &str,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooAuthExchange, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn ensure_token(
        &self,
        _user_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooToken, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        _page: usize,
        _per_page: usize,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooWorkoutList, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooWorkout, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<Option<crate::domain::wahoo::WahooWorkoutSummary>, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<Option<crate::domain::wahoo::WahooPlan>, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        _request: crate::domain::wahoo::WahooCreatePlan,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooPlan, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        _plan_id: i64,
        _request: crate::domain::wahoo::WahooUpdatePlan,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooPlan, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        _request: crate::domain::wahoo::WahooCreateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooWorkout, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn update_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
        _request: crate::domain::wahoo::WahooUpdateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<
        Result<crate::domain::wahoo::WahooWorkout, crate::domain::wahoo::WahooError>,
    > {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }

    fn download_workout_file(
        &self,
        _file_url: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<Vec<u8>, crate::domain::wahoo::WahooError>> {
        Box::pin(async { Err(crate::domain::wahoo::WahooError::NotConnected) })
    }
}

impl<Intervals, Entries, Projections, Syncs, Time>
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        NoopWahooUseCases,
        NoopPlannedWorkoutWahooSyncRepository,
        NoopUserSettingsRepository,
        NoopPlannedWorkoutTokenRepository,
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
            wahoo: NoopWahooUseCases,
            wahoo_syncs: NoopPlannedWorkoutWahooSyncRepository::default(),
            settings: NoopUserSettingsRepository,
            planned_workout_tokens: NoopPlannedWorkoutTokenRepository::default(),
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
        Refresh,
        Completed,
    >
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
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
    Wahoo: WahooUseCases + Clone,
    WahooSyncs: PlannedWorkoutWahooSyncRepository + Clone,
    Settings: UserSettingsRepository + Clone,
    Tokens: PlannedWorkoutTokenRepository + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    pub fn with_wahoo<NewWahoo, NewWahooSyncs, NewSettings>(
        self,
        wahoo: NewWahoo,
        wahoo_syncs: NewWahooSyncs,
        settings: NewSettings,
    ) -> CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        NewWahoo,
        NewWahooSyncs,
        NewSettings,
        Tokens,
        Refresh,
        Completed,
    >
    where
        NewWahoo: WahooUseCases + Clone,
        NewWahooSyncs: PlannedWorkoutWahooSyncRepository + Clone,
        NewSettings: UserSettingsRepository + Clone,
    {
        CalendarService {
            intervals: self.intervals,
            entries: self.entries,
            completed_workouts: self.completed_workouts,
            projections: self.projections,
            syncs: self.syncs,
            clock: self.clock,
            wahoo,
            wahoo_syncs,
            settings,
            planned_workout_tokens: self.planned_workout_tokens,
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
        Wahoo,
        WahooSyncs,
        Settings,
        NewTokens,
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
            wahoo: self.wahoo,
            wahoo_syncs: self.wahoo_syncs,
            settings: self.settings,
            planned_workout_tokens,
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
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
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
            wahoo: self.wahoo,
            wahoo_syncs: self.wahoo_syncs,
            settings: self.settings,
            planned_workout_tokens: self.planned_workout_tokens,
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
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
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
            wahoo: self.wahoo,
            wahoo_syncs: self.wahoo_syncs,
            settings: self.settings,
            planned_workout_tokens: self.planned_workout_tokens,
            refresh: self.refresh,
        }
    }
}

impl<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
        Refresh,
        Completed,
    > CalendarUseCases
    for CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
        Refresh,
        Completed,
    >
where
    Intervals: IntervalsUseCases + Clone,
    Entries: CalendarEntryViewRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Time: Clock + Clone,
    Wahoo: WahooUseCases + Clone,
    WahooSyncs: PlannedWorkoutWahooSyncRepository + Clone,
    Settings: UserSettingsRepository + Clone,
    Tokens: PlannedWorkoutTokenRepository + Clone,
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
