use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use chrono::{Duration, NaiveDate};
use futures::{stream, StreamExt};

use crate::domain::{
    identity::Clock,
    intervals::{Activity, DateRange, Event, IntervalsUseCases},
    llm::LlmError,
    races::RaceRepository,
    settings::UserSettingsUseCases,
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
use dates::{activity_date, epoch_seconds_to_date, event_date, intervals_status_message};
use history::build_recent_interval_blocks_by_activity_id;
use power::activity_has_required_detail;

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

const MAX_RECENT_ACTIVITY_FETCHES: usize = 4;
const STREAM_BUCKET_SIZE: usize = 5;
const MAX_CHUNKS_PER_WORKOUT: usize = 48;
const STABLE_FUTURE_EVENT_DAYS: i64 = 120;

#[derive(Clone)]
pub struct DefaultTrainingContextBuilder<Time>
where
    Time: Clock,
{
    settings_service: Arc<dyn UserSettingsUseCases>,
    intervals_service: Arc<dyn IntervalsUseCases>,
    workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
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
        intervals_service: Arc<dyn IntervalsUseCases>,
        workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
        clock: Time,
    ) -> Self {
        Self {
            settings_service,
            intervals_service,
            workout_summary_repository,
            race_repository: None,
            training_plan_projection_repository: None,
            clock,
        }
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

        let history_activities_result = self
            .intervals_service
            .list_activities(user_id, &activities_range)
            .await;
        let events_result = self
            .intervals_service
            .list_events(user_id, &events_range)
            .await;
        let stable_future_events_result = self
            .intervals_service
            .list_events(user_id, &stable_future_events_range)
            .await;

        let (history_activities, activities_status) = match history_activities_result {
            Ok(activities) => (activities, "ok".to_string()),
            Err(error) => (Vec::new(), intervals_status_message(&error)),
        };
        let (events, events_status) = match events_result {
            Ok(events) => (events, "ok".to_string()),
            Err(error) => (Vec::new(), intervals_status_message(&error)),
        };
        let stable_future_events = stable_future_events_result
            .unwrap_or_default()
            .into_iter()
            .filter(|event| {
                let date = event_date(event);
                date > focus_date && date <= stable_future_events_end
            })
            .collect::<Vec<_>>();

        let recent_activity_ids = history_activities
            .iter()
            .filter(|activity| {
                activity_date(activity) >= recent_start && activity_date(activity) <= focus_date
            })
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();

        let detailed_recent_activities = self
            .load_detailed_recent_activities(user_id, &history_activities, recent_start, focus_date)
            .await;
        let historical_activity_ids = history_activities
            .iter()
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();
        let summaries_by_id = self
            .load_rpe_by_workout_id(user_id, &recent_activity_ids, &events)
            .await;
        let workout_recaps_by_id = self
            .load_workout_recaps_by_workout_id(user_id, &historical_activity_ids, &events)
            .await;
        let projected_days = self.load_projected_day_contexts(user_id, focus_date).await;
        let races = self.load_race_contexts(user_id).await;
        let future_events = build_future_planned_event_contexts(&stable_future_events);
        let detailed_recent_activities_by_id = detailed_recent_activities
            .iter()
            .cloned()
            .map(|activity| (activity.id.clone(), activity))
            .collect::<HashMap<_, _>>();

        let recent_events = events
            .iter()
            .filter(|event| event_date(event) >= recent_start && event_date(event) <= focus_date)
            .cloned()
            .collect::<Vec<_>>();
        let upcoming_events = events
            .iter()
            .filter(|event| event_date(event) > focus_date && event_date(event) <= upcoming_end)
            .cloned()
            .collect::<Vec<_>>();
        let configured_ftp = settings
            .cycling
            .ftp_watts
            .and_then(|value| i32::try_from(value).ok());
        let matched_recent_workouts = build_event_activity_matches(
            &recent_events,
            &detailed_recent_activities,
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

    async fn load_detailed_recent_activities(
        &self,
        user_id: &str,
        activities: &[Activity],
        start: NaiveDate,
        end: NaiveDate,
    ) -> Vec<Activity> {
        let recent = activities
            .iter()
            .filter(|activity| activity_date(activity) >= start && activity_date(activity) <= end)
            .cloned()
            .collect::<Vec<_>>();

        stream::iter(recent)
            .map(|activity| async move {
                if activity_has_required_detail(&activity) {
                    return activity;
                }

                let fallback = activity.clone();
                match self
                    .intervals_service
                    .get_activity(user_id, &activity.id)
                    .await
                {
                    Ok(detailed) => detailed,
                    Err(_) => fallback,
                }
            })
            .buffer_unordered(MAX_RECENT_ACTIVITY_FETCHES)
            .collect()
            .await
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

        if let Ok(activity) = self
            .intervals_service
            .get_activity(user_id, workout_id)
            .await
        {
            return Some(activity_date(&activity));
        }

        let event_id = workout_id.parse::<i64>().ok()?;
        self.intervals_service
            .get_event(user_id, event_id)
            .await
            .ok()
            .map(|event| event_date(&event))
    }
}
