use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use chrono::{Duration, NaiveDate};

use crate::domain::{
    completed_workouts::{
        BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout, CompletedWorkoutError,
        CompletedWorkoutMetrics, CompletedWorkoutRepository, CompletedWorkoutSeries,
    },
    identity::Clock,
    intervals::{
        Activity, ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
        ActivityStream, ActivityZoneTime, DateRange, Event, EventCategory,
    },
    llm::LlmError,
    planned_workouts::{
        BoxFuture as PlannedWorkoutBoxFuture, PlannedWorkout, PlannedWorkoutError,
        PlannedWorkoutRepository,
    },
    races::RaceRepository,
    settings::UserSettingsUseCases,
    special_days::{
        BoxFuture as SpecialDayBoxFuture, SpecialDay, SpecialDayError, SpecialDayKind,
        SpecialDayRepository,
    },
    training_context::{model::*, packing::render_training_context},
    training_plan::TrainingPlanProjectionRepository,
    workout_summary::WorkoutSummaryRepository,
};

mod context;
mod dates;
mod history;
mod power;

#[cfg(test)]
mod tests;

use context::{
    build_event_activity_matches, build_future_planned_event_contexts, build_historical_context,
    build_recent_day_contexts, build_upcoming_day_contexts, infer_focus_kind,
    projected_interval_blocks, projected_raw_workout_doc, projected_workout_name,
    RecentWorkoutSummaryLookup,
};
use dates::{activity_date, epoch_seconds_to_date, event_date};
use history::build_recent_interval_blocks_by_activity_id;

pub trait TrainingContextBuilder: Send + Sync {
    fn build(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::llm::BoxFuture<Result<TrainingContextBuildResult, LlmError>>;

    fn build_athlete_summary_context(
        &self,
        user_id: &str,
    ) -> crate::domain::llm::BoxFuture<Result<TrainingContextBuildResult, LlmError>>;
}

const STREAM_BUCKET_SIZE: usize = 5;
const MAX_CHUNKS_PER_WORKOUT: usize = 48;
const STABLE_FUTURE_EVENT_DAYS: i64 = 120;

trait CompletedWorkoutReadPort: Send + Sync {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>;

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>;
}

impl<Repository> CompletedWorkoutReadPort for Repository
where
    Repository: CompletedWorkoutRepository,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        CompletedWorkoutRepository::list_by_user_id(self, user_id)
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        CompletedWorkoutRepository::list_by_user_id_and_date_range(self, user_id, oldest, newest)
    }
}

trait PlannedWorkoutReadPort: Send + Sync {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>;

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>;
}

impl<Repository> PlannedWorkoutReadPort for Repository
where
    Repository: PlannedWorkoutRepository,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        PlannedWorkoutRepository::list_by_user_id(self, user_id)
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        PlannedWorkoutRepository::list_by_user_id_and_date_range(self, user_id, oldest, newest)
    }
}

trait SpecialDayReadPort: Send + Sync {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> SpecialDayBoxFuture<Result<Vec<SpecialDay>, SpecialDayError>>;

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> SpecialDayBoxFuture<Result<Vec<SpecialDay>, SpecialDayError>>;
}

impl<Repository> SpecialDayReadPort for Repository
where
    Repository: SpecialDayRepository,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> SpecialDayBoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        SpecialDayRepository::list_by_user_id(self, user_id)
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> SpecialDayBoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        SpecialDayRepository::list_by_user_id_and_date_range(self, user_id, oldest, newest)
    }
}

#[derive(Clone)]
pub struct DefaultTrainingContextBuilder<Time>
where
    Time: Clock,
{
    settings_service: Arc<dyn UserSettingsUseCases>,
    workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
    completed_workout_repository: Option<Arc<dyn CompletedWorkoutReadPort>>,
    planned_workout_repository: Option<Arc<dyn PlannedWorkoutReadPort>>,
    special_day_repository: Option<Arc<dyn SpecialDayReadPort>>,
    race_repository: Option<Arc<dyn RaceRepository>>,
    training_plan_projection_repository: Option<Arc<dyn TrainingPlanProjectionRepository>>,
    clock: Time,
}

impl<Time> DefaultTrainingContextBuilder<Time>
where
    Time: Clock,
{
    pub fn new(
        settings_service: Arc<dyn UserSettingsUseCases>,
        workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
        clock: Time,
    ) -> Self {
        Self {
            settings_service,
            workout_summary_repository,
            completed_workout_repository: None,
            planned_workout_repository: None,
            special_day_repository: None,
            race_repository: None,
            training_plan_projection_repository: None,
            clock,
        }
    }

    pub fn with_completed_workout_repository<Repository>(mut self, repository: Repository) -> Self
    where
        Repository: CompletedWorkoutRepository,
    {
        self.completed_workout_repository = Some(Arc::new(repository));
        self
    }

    pub fn with_planned_workout_repository<Repository>(mut self, repository: Repository) -> Self
    where
        Repository: PlannedWorkoutRepository,
    {
        self.planned_workout_repository = Some(Arc::new(repository));
        self
    }

    pub fn with_special_day_repository<Repository>(mut self, repository: Repository) -> Self
    where
        Repository: SpecialDayRepository,
    {
        self.special_day_repository = Some(Arc::new(repository));
        self
    }

    pub fn with_race_repository(mut self, race_repository: Arc<dyn RaceRepository>) -> Self {
        self.race_repository = Some(race_repository);
        self
    }

    pub fn with_training_plan_projection_repository(
        mut self,
        training_plan_projection_repository: Arc<dyn TrainingPlanProjectionRepository>,
    ) -> Self {
        self.training_plan_projection_repository = Some(training_plan_projection_repository);
        self
    }
}

impl<Time> TrainingContextBuilder for DefaultTrainingContextBuilder<Time>
where
    Time: Clock,
{
    fn build(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::llm::BoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let builder = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move { builder.build_impl(&user_id, &workout_id).await })
    }

    fn build_athlete_summary_context(
        &self,
        user_id: &str,
    ) -> crate::domain::llm::BoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let builder = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { builder.build_impl(&user_id, "athlete-summary").await })
    }
}

impl<Time> DefaultTrainingContextBuilder<Time>
where
    Time: Clock,
{
    async fn build_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<TrainingContextBuildResult, LlmError> {
        let settings = self
            .settings_service
            .get_settings(user_id)
            .await
            .map_err(|error| LlmError::Internal(error.to_string()))?;
        let today = epoch_seconds_to_date(self.clock.now_epoch_seconds());
        let focus_date = self
            .resolve_focus_date(user_id, workout_id)
            .await
            .unwrap_or(today);
        let history_trend_days = 24 * 7;
        let history_warmup_days = 120;
        let history_start =
            focus_date - Duration::days((history_trend_days + history_warmup_days - 1) as i64);
        let recent_start = focus_date - Duration::days(13);
        let upcoming_end = focus_date + Duration::days(14);
        let stable_future_events_start = focus_date + Duration::days(1);
        let stable_future_events_end = focus_date + Duration::days(STABLE_FUTURE_EVENT_DAYS);

        let activities_range = DateRange {
            oldest: history_start.format("%Y-%m-%d").to_string(),
            newest: focus_date.format("%Y-%m-%d").to_string(),
        };
        let events_range = DateRange {
            oldest: recent_start.format("%Y-%m-%d").to_string(),
            newest: upcoming_end.format("%Y-%m-%d").to_string(),
        };
        let stable_future_events_range = DateRange {
            oldest: stable_future_events_start.format("%Y-%m-%d").to_string(),
            newest: stable_future_events_end.format("%Y-%m-%d").to_string(),
        };

        let (history_completed_workouts, activities_status) = match self
            .list_completed_workouts(user_id, &activities_range.oldest, &activities_range.newest)
            .await
        {
            Ok(workouts) => (workouts, "ok".to_string()),
            Err(_) => (Vec::new(), "internal_error".to_string()),
        };
        let history_activities = history_completed_workouts
            .iter()
            .map(map_completed_workout_to_activity)
            .collect::<Vec<_>>();

        let (planned_workouts, special_days, events_status) = match self
            .load_event_sources(
                user_id,
                &events_range.oldest,
                &stable_future_events_range.newest,
            )
            .await
        {
            Ok((planned_workouts, special_days)) => {
                (planned_workouts, special_days, "ok".to_string())
            }
            Err(_) => (Vec::new(), Vec::new(), "internal_error".to_string()),
        };
        let planned_events_by_id = planned_workouts
            .iter()
            .map(|workout| {
                (
                    workout.planned_workout_id.clone(),
                    map_planned_workout_to_event(workout),
                )
            })
            .collect::<HashMap<_, _>>();
        let direct_event_matches =
            build_direct_event_matches(&history_completed_workouts, &planned_events_by_id);
        let all_events = build_local_events(&planned_workouts, &special_days);
        let stable_future_events = all_events
            .iter()
            .filter(|event| {
                let date = event_date(event);
                date > focus_date && date <= stable_future_events_end
            })
            .cloned()
            .collect::<Vec<_>>();
        let configured_ftp = settings
            .cycling
            .ftp_watts
            .and_then(|value| i32::try_from(value).ok());

        let recent_activity_ids = history_activities
            .iter()
            .filter(|activity| {
                activity_date(activity) >= recent_start && activity_date(activity) <= focus_date
            })
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();

        let detailed_recent_activities =
            self.load_detailed_recent_activities(&history_activities, recent_start, focus_date);
        let historical_activity_ids = history_activities
            .iter()
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();
        let summaries_by_id = self
            .load_rpe_by_workout_id(user_id, &recent_activity_ids, &all_events)
            .await;
        let workout_recaps_by_id = self
            .load_workout_recaps_by_workout_id(user_id, &historical_activity_ids, &all_events)
            .await;
        let projected_days = self.load_projected_day_contexts(user_id, focus_date).await;
        let races = self.load_race_contexts(user_id).await;
        let future_events =
            build_future_planned_event_contexts(&stable_future_events, configured_ftp);
        let detailed_recent_activities_by_id = detailed_recent_activities
            .iter()
            .cloned()
            .map(|activity| (activity.id.clone(), activity))
            .collect::<HashMap<_, _>>();

        let recent_events = all_events
            .iter()
            .filter(|event| event_date(event) >= recent_start && event_date(event) <= focus_date)
            .cloned()
            .collect::<Vec<_>>();
        let upcoming_events = all_events
            .iter()
            .filter(|event| event_date(event) > focus_date && event_date(event) <= upcoming_end)
            .cloned()
            .collect::<Vec<_>>();
        let matched_recent_workouts = build_event_activity_matches(
            &recent_events,
            &detailed_recent_activities,
            &direct_event_matches,
            configured_ftp,
        );
        let recent_interval_blocks_by_activity_id = build_recent_interval_blocks_by_activity_id(
            &detailed_recent_activities,
            &matched_recent_workouts,
            configured_ftp,
        );

        let availability_configured = settings.availability.is_configured();
        let weekly_availability = if availability_configured {
            settings
                .availability
                .days
                .into_iter()
                .map(|day| WeeklyAvailabilityContext {
                    weekday: day.weekday,
                    available: day.available,
                    max_duration_minutes: day.max_duration_minutes,
                })
                .collect()
        } else {
            Vec::new()
        };

        let profile = AthleteProfileContext {
            full_name: settings.cycling.full_name,
            age: settings.cycling.age,
            height_cm: settings.cycling.height_cm,
            weight_kg: settings.cycling.weight_kg,
            ftp_watts: settings.cycling.ftp_watts,
            hr_max_bpm: settings.cycling.hr_max_bpm,
            vo2_max: settings.cycling.vo2_max,
            athlete_prompt: settings.cycling.athlete_prompt,
            medications: settings.cycling.medications,
            athlete_notes: settings.cycling.athlete_notes,
            availability_configured,
            weekly_availability,
        };

        let history = build_historical_context(
            history_start,
            focus_date,
            &history_activities,
            &detailed_recent_activities_by_id,
            &workout_recaps_by_id,
            configured_ftp,
            &recent_interval_blocks_by_activity_id,
        );
        let recent_days = build_recent_day_contexts(
            recent_start,
            focus_date,
            &recent_events,
            &detailed_recent_activities,
            RecentWorkoutSummaryLookup {
                rpe_by_workout_id: &summaries_by_id,
                recap_by_workout_id: &workout_recaps_by_id,
            },
            &matched_recent_workouts,
            configured_ftp,
        );
        let upcoming_days = build_upcoming_day_contexts(
            focus_date + Duration::days(1),
            upcoming_end,
            &upcoming_events,
        );
        let focus_kind = infer_focus_kind(workout_id, &recent_days, &upcoming_days);

        let context = TrainingContext {
            generated_at_epoch_seconds: self.clock.now_epoch_seconds(),
            focus_workout_id: if focus_kind == "summary" {
                None
            } else {
                Some(workout_id.to_string())
            },
            focus_kind,
            intervals_status: IntervalsStatusContext {
                activities: activities_status,
                events: events_status,
            },
            profile,
            races,
            future_events,
            history,
            recent_days,
            upcoming_days,
            projected_days,
        };
        let rendered = render_training_context(&context);

        Ok(TrainingContextBuildResult { context, rendered })
    }

    fn load_detailed_recent_activities(
        &self,
        activities: &[Activity],
        start: NaiveDate,
        end: NaiveDate,
    ) -> Vec<Activity> {
        activities
            .iter()
            .filter(|activity| activity_date(activity) >= start && activity_date(activity) <= end)
            .cloned()
            .collect()
    }

    async fn load_rpe_by_workout_id(
        &self,
        user_id: &str,
        activity_ids: &[String],
        events: &[Event],
    ) -> HashMap<String, u8> {
        let mut ids = activity_ids.to_vec();
        ids.extend(events.iter().map(|event| event.id.to_string()));
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return HashMap::new();
        }

        match self
            .workout_summary_repository
            .find_by_user_id_and_workout_ids(user_id, ids)
            .await
        {
            Ok(summaries) => summaries
                .into_iter()
                .filter_map(|summary| summary.rpe.map(|rpe| (summary.workout_id, rpe)))
                .collect(),
            Err(_) => HashMap::new(),
        }
    }

    async fn load_workout_recaps_by_workout_id(
        &self,
        user_id: &str,
        activity_ids: &[String],
        events: &[Event],
    ) -> HashMap<String, String> {
        let mut ids = activity_ids.to_vec();
        ids.extend(events.iter().map(|event| event.id.to_string()));
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return HashMap::new();
        }

        match self
            .workout_summary_repository
            .find_by_user_id_and_workout_ids(user_id, ids)
            .await
        {
            Ok(summaries) => summaries
                .into_iter()
                .filter_map(|summary| {
                    summary
                        .workout_recap_text
                        .map(|text| (summary.workout_id, text))
                })
                .collect(),
            Err(_) => HashMap::new(),
        }
    }

    async fn load_projected_day_contexts(
        &self,
        user_id: &str,
        focus_date: NaiveDate,
    ) -> Vec<ProjectedDayContext> {
        let Some(repository) = &self.training_plan_projection_repository else {
            return Vec::new();
        };

        let projected_days = match repository.list_active_by_user_id(user_id).await {
            Ok(projected_days) => projected_days,
            Err(_) => return Vec::new(),
        };
        let focus_date = focus_date.format("%Y-%m-%d").to_string();

        let mut grouped = BTreeMap::<String, Vec<ProjectedWorkoutContext>>::new();
        for day in projected_days
            .into_iter()
            .filter(|day| day.date > focus_date)
        {
            grouped
                .entry(day.date.clone())
                .or_default()
                .push(ProjectedWorkoutContext {
                    source_workout_id: day.workout_id,
                    start_date_local: format!("{}T00:00:00", day.date),
                    name: day.workout.as_ref().and_then(projected_workout_name),
                    interval_blocks: day
                        .workout
                        .as_ref()
                        .map(projected_interval_blocks)
                        .unwrap_or_default(),
                    raw_workout_doc: day.workout.as_ref().map(projected_raw_workout_doc),
                    rest_day: day.rest_day,
                });
        }

        grouped
            .into_iter()
            .map(|(date, workouts)| ProjectedDayContext { date, workouts })
            .collect()
    }

    async fn load_race_contexts(&self, user_id: &str) -> Vec<RaceContext> {
        let Some(repository) = &self.race_repository else {
            return Vec::new();
        };

        match repository.list_by_user_id(user_id).await {
            Ok(races) => {
                let mut contexts = races
                    .into_iter()
                    .map(|race| RaceContext {
                        race_id: race.race_id,
                        date: race.date,
                        name: race.name,
                        distance_meters: race.distance_meters,
                        discipline: race.discipline.as_str().to_string(),
                        priority: race.priority.as_str().to_string(),
                    })
                    .collect::<Vec<_>>();
                contexts.sort_by(|left, right| {
                    left.date
                        .cmp(&right.date)
                        .then_with(|| left.name.cmp(&right.name))
                });
                contexts
            }
            Err(_) => Vec::new(),
        }
    }

    async fn resolve_focus_date(&self, user_id: &str, workout_id: &str) -> Option<NaiveDate> {
        if workout_id == "athlete-summary" {
            return None;
        }

        if let Some(repository) = &self.completed_workout_repository {
            if let Ok(workouts) = repository.list_by_user_id(user_id).await {
                if let Some(workout) = workouts.into_iter().find(|workout| {
                    workout.completed_workout_id == workout_id
                        || legacy_activity_id(&workout.completed_workout_id) == workout_id
                }) {
                    return Some(dates::parse_date(date_key(&workout.start_date_local)));
                }
            }
        }

        if let Some(repository) = &self.planned_workout_repository {
            if let Ok(workouts) = repository.list_by_user_id(user_id).await {
                if let Some(workout) = workouts
                    .into_iter()
                    .find(|workout| planned_workout_matches_identity(workout, workout_id))
                {
                    return Some(dates::parse_date(&workout.date));
                }
            }
        }

        if let Some(repository) = &self.special_day_repository {
            if let Ok(days) = repository.list_by_user_id(user_id).await {
                if let Some(day) = days
                    .into_iter()
                    .find(|day| special_day_matches_identity(day, workout_id))
                {
                    return Some(dates::parse_date(&day.date));
                }
            }
        }

        let repository = self.training_plan_projection_repository.as_ref()?;
        repository
            .list_active_by_user_id(user_id)
            .await
            .ok()?
            .into_iter()
            .find(|day| {
                day.workout_id == workout_id
                    || format!("{}:{}", day.operation_key, day.date) == workout_id
            })
            .map(|day| dates::parse_date(&day.date))
    }

    async fn list_completed_workouts(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> Result<Vec<CompletedWorkout>, CompletedWorkoutError> {
        let Some(repository) = &self.completed_workout_repository else {
            return Ok(Vec::new());
        };

        repository
            .list_by_user_id_and_date_range(user_id, oldest, newest)
            .await
    }

    async fn load_event_sources(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> Result<(Vec<PlannedWorkout>, Vec<SpecialDay>), LlmError> {
        let planned_workouts = match &self.planned_workout_repository {
            Some(repository) => repository
                .list_by_user_id_and_date_range(user_id, oldest, newest)
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?,
            None => Vec::new(),
        };
        let special_days = match &self.special_day_repository {
            Some(repository) => repository
                .list_by_user_id_and_date_range(user_id, oldest, newest)
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?,
            None => Vec::new(),
        };

        Ok((planned_workouts, special_days))
    }
}

fn build_local_events(
    planned_workouts: &[PlannedWorkout],
    special_days: &[SpecialDay],
) -> Vec<Event> {
    let mut events = planned_workouts
        .iter()
        .filter(|workout| !is_projected_planned_workout(workout))
        .map(map_planned_workout_to_event)
        .collect::<Vec<_>>();
    events.extend(special_days.iter().map(map_special_day_to_event));
    events.sort_by(|left, right| {
        left.start_date_local
            .cmp(&right.start_date_local)
            .then_with(|| left.id.cmp(&right.id))
    });
    events
}

fn build_direct_event_matches(
    completed_workouts: &[CompletedWorkout],
    planned_events_by_id: &HashMap<String, Event>,
) -> HashMap<String, Event> {
    completed_workouts
        .iter()
        .filter_map(|workout| {
            workout
                .planned_workout_id
                .as_ref()
                .and_then(|planned_workout_id| {
                    planned_events_by_id
                        .get(planned_workout_id)
                        .cloned()
                        .map(|event| {
                            (
                                legacy_activity_id(&workout.completed_workout_id).to_string(),
                                event,
                            )
                        })
                })
        })
        .collect()
}

fn map_planned_workout_to_event(workout: &PlannedWorkout) -> Event {
    Event {
        id: canonical_event_id(&workout.planned_workout_id, &workout.date),
        start_date_local: format!("{}T00:00:00", workout.date),
        event_type: workout.event_type.clone(),
        name: workout
            .name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                workout.workout.lines.iter().find_map(|line| match line {
                    crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => {
                        Some(text.text.clone())
                    }
                    _ => None,
                })
            }),
        category: EventCategory::Workout,
        description: workout.description.clone(),
        indoor: false,
        color: None,
        workout_doc: Some(serialize_canonical_planned_workout(workout)),
    }
}

fn map_special_day_to_event(day: &SpecialDay) -> Event {
    Event {
        id: canonical_event_id(&day.special_day_id, &day.date),
        start_date_local: format!("{}T00:00:00", day.date),
        event_type: None,
        name: day
            .title
            .clone()
            .or_else(|| Some(default_special_day_name(&day.kind))),
        category: EventCategory::Note,
        description: day.description.clone(),
        indoor: false,
        color: None,
        workout_doc: None,
    }
}

fn map_completed_workout_to_activity(workout: &CompletedWorkout) -> Activity {
    Activity {
        id: legacy_activity_id(&workout.completed_workout_id).to_string(),
        athlete_id: None,
        start_date_local: workout.start_date_local.clone(),
        start_date: None,
        name: workout.name.clone(),
        description: workout.description.clone(),
        activity_type: workout.activity_type.clone(),
        source: Some("canonical".to_string()),
        external_id: None,
        device_name: None,
        distance_meters: workout.distance_meters,
        moving_time_seconds: workout.duration_seconds,
        elapsed_time_seconds: workout.duration_seconds,
        total_elevation_gain_meters: None,
        total_elevation_loss_meters: None,
        average_speed_mps: None,
        max_speed_mps: None,
        average_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        average_cadence_rpm: None,
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: workout
            .details
            .streams
            .iter()
            .any(|stream| stream.stream_type.eq_ignore_ascii_case("heartrate")),
        stream_types: workout
            .details
            .streams
            .iter()
            .map(|stream| stream.stream_type.clone())
            .collect(),
        tags: Vec::new(),
        metrics: map_completed_metrics(&workout.metrics),
        details: ActivityDetails {
            intervals: workout
                .details
                .intervals
                .iter()
                .map(|interval| ActivityInterval {
                    id: interval.id,
                    label: interval.label.clone(),
                    interval_type: interval.interval_type.clone(),
                    group_id: interval.group_id.clone(),
                    start_index: interval.start_index,
                    end_index: interval.end_index,
                    start_time_seconds: interval.start_time_seconds,
                    end_time_seconds: interval.end_time_seconds,
                    moving_time_seconds: interval.moving_time_seconds,
                    elapsed_time_seconds: interval.elapsed_time_seconds,
                    distance_meters: interval.distance_meters,
                    average_power_watts: interval.average_power_watts,
                    normalized_power_watts: interval.normalized_power_watts,
                    training_stress_score: interval.training_stress_score,
                    average_heart_rate_bpm: interval.average_heart_rate_bpm,
                    average_cadence_rpm: interval.average_cadence_rpm,
                    average_speed_mps: interval.average_speed_mps,
                    average_stride_meters: interval.average_stride_meters,
                    zone: interval.zone,
                })
                .collect(),
            interval_groups: workout
                .details
                .interval_groups
                .iter()
                .map(|group| ActivityIntervalGroup {
                    id: group.id.clone(),
                    count: group.count,
                    start_index: group.start_index,
                    moving_time_seconds: group.moving_time_seconds,
                    elapsed_time_seconds: group.elapsed_time_seconds,
                    distance_meters: group.distance_meters,
                    average_power_watts: group.average_power_watts,
                    normalized_power_watts: group.normalized_power_watts,
                    training_stress_score: group.training_stress_score,
                    average_heart_rate_bpm: group.average_heart_rate_bpm,
                    average_cadence_rpm: group.average_cadence_rpm,
                    average_speed_mps: group.average_speed_mps,
                    average_stride_meters: group.average_stride_meters,
                })
                .collect(),
            streams: workout
                .details
                .streams
                .iter()
                .map(map_completed_stream)
                .collect(),
            interval_summary: workout.details.interval_summary.clone(),
            skyline_chart: workout.details.skyline_chart.clone(),
            power_zone_times: workout
                .details
                .power_zone_times
                .iter()
                .map(|zone| ActivityZoneTime {
                    zone_id: zone.zone_id.clone(),
                    seconds: zone.seconds,
                })
                .collect(),
            heart_rate_zone_times: workout.details.heart_rate_zone_times.clone(),
            pace_zone_times: workout.details.pace_zone_times.clone(),
            gap_zone_times: workout.details.gap_zone_times.clone(),
        },
        details_unavailable_reason: None,
    }
}

fn map_completed_metrics(metrics: &CompletedWorkoutMetrics) -> ActivityMetrics {
    ActivityMetrics {
        training_stress_score: metrics.training_stress_score,
        normalized_power_watts: metrics.normalized_power_watts,
        intensity_factor: metrics.intensity_factor,
        efficiency_factor: metrics.efficiency_factor,
        variability_index: metrics.variability_index,
        average_power_watts: metrics.average_power_watts,
        ftp_watts: metrics.ftp_watts,
        total_work_joules: metrics.total_work_joules,
        calories: metrics.calories,
        trimp: metrics.trimp,
        power_load: metrics.power_load,
        heart_rate_load: metrics.heart_rate_load,
        pace_load: metrics.pace_load,
        strain_score: metrics.strain_score,
    }
}

fn map_completed_stream(
    stream: &crate::domain::completed_workouts::CompletedWorkoutStream,
) -> ActivityStream {
    ActivityStream {
        stream_type: stream.stream_type.clone(),
        name: stream.name.clone(),
        data: map_completed_series(stream.primary_series.as_ref()),
        data2: map_completed_series(stream.secondary_series.as_ref()),
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}

fn map_completed_series(series: Option<&CompletedWorkoutSeries>) -> Option<serde_json::Value> {
    match series? {
        CompletedWorkoutSeries::Integers(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Floats(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Bools(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Strings(values) => Some(serde_json::json!(values)),
    }
}

fn serialize_canonical_planned_workout(workout: &PlannedWorkout) -> String {
    let structured = crate::domain::intervals::PlannedWorkout {
        lines: workout
            .workout
            .lines
            .iter()
            .cloned()
            .map(map_canonical_line_to_intervals_line)
            .collect(),
    };

    crate::domain::intervals::serialize_planned_workout(&structured)
}

fn map_canonical_line_to_intervals_line(
    line: crate::domain::planned_workouts::PlannedWorkoutLine,
) -> crate::domain::intervals::PlannedWorkoutLine {
    match line {
        crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => {
            crate::domain::intervals::PlannedWorkoutLine::Text(
                crate::domain::intervals::PlannedWorkoutText { text: text.text },
            )
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Repeat(repeat) => {
            crate::domain::intervals::PlannedWorkoutLine::Repeat(
                crate::domain::intervals::PlannedWorkoutRepeat {
                    title: repeat.title,
                    count: repeat.count,
                },
            )
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Step(step) => {
            crate::domain::intervals::PlannedWorkoutLine::Step(
                crate::domain::intervals::PlannedWorkoutStep {
                    duration_seconds: step.duration_seconds,
                    kind: match step.kind {
                        crate::domain::planned_workouts::PlannedWorkoutStepKind::Steady => {
                            crate::domain::intervals::PlannedWorkoutStepKind::Steady
                        }
                        crate::domain::planned_workouts::PlannedWorkoutStepKind::Ramp => {
                            crate::domain::intervals::PlannedWorkoutStepKind::Ramp
                        }
                    },
                    target: match step.target {
                        crate::domain::planned_workouts::PlannedWorkoutTarget::PercentFtp {
                            min,
                            max,
                        } => {
                            crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max }
                        }
                        crate::domain::planned_workouts::PlannedWorkoutTarget::WattsRange {
                            min,
                            max,
                        } => {
                            crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max }
                        }
                    },
                },
            )
        }
    }
}

fn legacy_activity_id(completed_workout_id: &str) -> &str {
    completed_workout_id
        .strip_prefix("intervals-activity:")
        .unwrap_or(completed_workout_id)
}

fn canonical_event_id(entity_id: &str, date: &str) -> i64 {
    entity_id
        .strip_prefix("intervals-event:")
        .or_else(|| entity_id.strip_prefix("intervals-special-day:"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(|| synthetic_event_id(entity_id, date))
}

fn synthetic_event_id(entity_id: &str, date: &str) -> i64 {
    use sha2::{Digest, Sha256};

    const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    let digest = Sha256::digest(format!("{entity_id}:{date}"));
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_be_bytes(bytes);
    ((value % MAX_JS_SAFE_INTEGER) + 1) as i64
}

fn is_projected_planned_workout(workout: &PlannedWorkout) -> bool {
    workout
        .planned_workout_id
        .rsplit_once(':')
        .is_some_and(|(_, projected_date)| projected_date == workout.date)
}

fn planned_workout_matches_identity(workout: &PlannedWorkout, workout_id: &str) -> bool {
    workout.planned_workout_id == workout_id
        || workout_id.parse::<i64>().ok().is_some_and(|event_id| {
            event_id == canonical_event_id(&workout.planned_workout_id, &workout.date)
        })
}

fn special_day_matches_identity(day: &SpecialDay, workout_id: &str) -> bool {
    day.special_day_id == workout_id
        || workout_id
            .parse::<i64>()
            .ok()
            .is_some_and(|event_id| event_id == canonical_event_id(&day.special_day_id, &day.date))
}

fn default_special_day_name(kind: &SpecialDayKind) -> String {
    match kind {
        SpecialDayKind::Illness => "Illness".to_string(),
        SpecialDayKind::Travel => "Travel".to_string(),
        SpecialDayKind::Blocked => "Blocked day".to_string(),
        SpecialDayKind::Note => "Note".to_string(),
        SpecialDayKind::Other => "Special day".to_string(),
    }
}

fn date_key(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}
