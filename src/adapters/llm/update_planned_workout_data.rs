use std::{future::Future, pin::Pin};

use crate::domain::{
    llm_tools::UpdatePlannedWorkoutDataPort,
    planned_workouts::{
        PlannedWorkoutUpdateService, UpdatePlannedWorkoutCommand, UpdatePlannedWorkoutError,
        UpdatePlannedWorkoutOutcome,
    },
};

pub trait UpdatePlannedWorkoutUseCases: Send + Sync + 'static {
    fn update_planned_workout(
        &self,
        command: UpdatePlannedWorkoutCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdatePlannedWorkoutOutcome, UpdatePlannedWorkoutError>>
                + Send,
        >,
    >;
}

pub struct UpdatePlannedWorkoutDataAdapter<Service> {
    service: Service,
}

impl<Service> UpdatePlannedWorkoutDataAdapter<Service> {
    pub fn new(service: Service) -> Self {
        Self { service }
    }
}

impl<Service> UpdatePlannedWorkoutDataPort for UpdatePlannedWorkoutDataAdapter<Service>
where
    Service: UpdatePlannedWorkoutUseCases,
{
    fn update_planned_workout(
        &self,
        command: UpdatePlannedWorkoutCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdatePlannedWorkoutOutcome, UpdatePlannedWorkoutError>>
                + Send,
        >,
    > {
        self.service.update_planned_workout(command)
    }
}

impl<Planned, SyncStates, Intervals, Wahoo, Settings, Tokens, Refresh, Time>
    UpdatePlannedWorkoutUseCases
    for PlannedWorkoutUpdateService<
        Planned,
        SyncStates,
        Intervals,
        Wahoo,
        Settings,
        Tokens,
        Refresh,
        Time,
    >
where
    Planned: crate::domain::planned_workouts::PlannedWorkoutRepository + 'static,
    SyncStates: crate::domain::external_sync::ExternalSyncStateRepository + 'static,
    Intervals: crate::domain::intervals::IntervalsUseCases + Clone + 'static,
    Wahoo: crate::domain::wahoo::WahooUseCases + Clone + 'static,
    Settings: crate::domain::settings::UserSettingsRepository + 'static,
    Tokens: crate::domain::planned_workout_tokens::PlannedWorkoutTokenRepository + 'static,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort + 'static,
    Time: crate::domain::identity::Clock + 'static,
{
    fn update_planned_workout(
        &self,
        command: UpdatePlannedWorkoutCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdatePlannedWorkoutOutcome, UpdatePlannedWorkoutError>>
                + Send,
        >,
    > {
        let service: PlannedWorkoutUpdateService<
            Planned,
            SyncStates,
            Intervals,
            Wahoo,
            Settings,
            Tokens,
            Refresh,
            Time,
        > = (*self).clone();
        Box::pin(async move { service.update_planned_workout(command).await })
    }
}
