use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use chrono::{Duration, NaiveDate, Utc};
use futures::{stream, StreamExt};

use crate::domain::{
    identity::Clock,
    intervals::{
        find_best_activity_match, parse_workout_doc, Activity, ActivityStream, DateRange, Event,
        EventCategory, IntervalsError, IntervalsUseCases, WorkoutSegment,
    },
    llm::LlmError,
    settings::UserSettingsUseCases,
    training_context::{model::*, packing::render_training_context},
    workout_summary::WorkoutSummaryRepository,
};

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

#[derive(Clone)]
pub struct DefaultTrainingContextBuilder<Time>
where
    Time: Clock,
{
    settings_service: Arc<dyn UserSettingsUseCases>,
    intervals_service: Arc<dyn IntervalsUseCases>,
    workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
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
            clock,
        }
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
        let history_trend_days = 24 * 7;
        let history_warmup_days = 120;
        let history_start =
            today - Duration::days((history_trend_days + history_warmup_days - 1) as i64);
        let recent_start = today - Duration::days(13);
        let upcoming_end = today + Duration::days(14);

        let activities_range = DateRange {
            oldest: history_start.format("%Y-%m-%d").to_string(),
            newest: today.format("%Y-%m-%d").to_string(),
        };
        let events_range = DateRange {
            oldest: recent_start.format("%Y-%m-%d").to_string(),
            newest: upcoming_end.format("%Y-%m-%d").to_string(),
        };

        let history_activities_result = self
            .intervals_service
            .list_activities(user_id, &activities_range)
            .await;
        let events_result = self
            .intervals_service
            .list_events(user_id, &events_range)
            .await;

        let (history_activities, activities_status) = match history_activities_result {
            Ok(activities) => (activities, "ok".to_string()),
            Err(error) => (Vec::new(), intervals_status_message(&error)),
        };
        let (events, events_status) = match events_result {
            Ok(events) => (events, "ok".to_string()),
            Err(error) => (Vec::new(), intervals_status_message(&error)),
        };

        let recent_activity_ids = history_activities
            .iter()
            .filter(|activity| {
                activity_date(activity) >= recent_start && activity_date(activity) <= today
            })
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();

        let detailed_recent_activities = self
            .load_detailed_recent_activities(user_id, &history_activities, recent_start, today)
            .await;
        let summaries_by_id = self
            .load_recent_rpe_by_workout_id(user_id, &recent_activity_ids, &events)
            .await;

        let recent_events = events
            .iter()
            .filter(|event| event_date(event) >= recent_start && event_date(event) <= today)
            .cloned()
            .collect::<Vec<_>>();
        let upcoming_events = events
            .iter()
            .filter(|event| event_date(event) > today && event_date(event) <= upcoming_end)
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
        );

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
        };

        let history = build_historical_context(
            history_start,
            today,
            &history_activities,
            &recent_interval_blocks_by_activity_id,
        );
        let recent_days = build_recent_day_contexts(
            recent_start,
            today,
            &recent_events,
            &detailed_recent_activities,
            &summaries_by_id,
            &matched_recent_workouts,
        );
        let upcoming_days =
            build_upcoming_day_contexts(today + Duration::days(1), upcoming_end, &upcoming_events);
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
            history,
            recent_days,
            upcoming_days,
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

    async fn load_recent_rpe_by_workout_id(
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
}

fn build_historical_context(
    start: NaiveDate,
    end: NaiveDate,
    activities: &[Activity],
    recent_interval_blocks_by_activity_id: &HashMap<String, Vec<PlannedWorkoutBlockContext>>,
) -> HistoricalTrainingContext {
    let workouts = activities
        .iter()
        .map(|activity| HistoricalWorkoutContext {
            date: date_key(&activity.start_date_local),
            activity_id: activity.id.clone(),
            name: activity.name.clone(),
            activity_type: activity.activity_type.clone(),
            duration_seconds: activity
                .elapsed_time_seconds
                .or(activity.moving_time_seconds),
            training_stress_score: activity.metrics.training_stress_score,
            intensity_factor: activity.metrics.intensity_factor,
            efficiency_factor: activity.metrics.efficiency_factor,
            normalized_power_watts: activity.metrics.normalized_power_watts,
            ftp_watts: activity.metrics.ftp_watts,
            variability_index: activity.metrics.variability_index,
            interval_blocks: recent_interval_blocks_by_activity_id
                .get(&activity.id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    let total_tss = activities
        .iter()
        .filter_map(|activity| activity.metrics.training_stress_score)
        .sum::<i32>();
    let daily_tss = build_daily_tss_map(start, end, activities);
    let load_trend = build_load_trend(&daily_tss, 42, 42);
    let ctl = ewma_latest(&daily_tss, 42);
    let atl = ewma_latest(&daily_tss, 7);
    let tsb = match (ctl, atl) {
        (Some(ctl), Some(atl)) => Some(round_to_2(ctl - atl)),
        _ => None,
    };
    let ftp_values = activities
        .iter()
        .filter_map(|activity| activity.metrics.ftp_watts)
        .collect::<Vec<_>>();
    let ftp_current = ftp_values.last().copied();
    let ftp_change = ftp_current
        .zip(ftp_values.first().copied())
        .map(|(latest, earliest)| latest - earliest);
    let average_tss_7d = average_recent_tss(&daily_tss, 7);
    let average_tss_28d = average_recent_tss(&daily_tss, 28);
    let recent_28 = recent_slice(activities, end - Duration::days(27));
    let average_if_28d = average_metric(
        recent_28
            .iter()
            .filter_map(|activity| activity.metrics.intensity_factor),
    );
    let average_ef_28d = average_metric(
        recent_28
            .iter()
            .filter_map(|activity| activity.metrics.efficiency_factor),
    );

    HistoricalTrainingContext {
        window_start: start.format("%Y-%m-%d").to_string(),
        window_end: end.format("%Y-%m-%d").to_string(),
        activity_count: activities.len(),
        total_tss,
        ctl,
        atl,
        tsb,
        ftp_current,
        ftp_change,
        average_tss_7d,
        average_tss_28d,
        average_if_28d,
        average_ef_28d,
        load_trend,
        workouts,
    }
}

fn build_recent_day_contexts(
    start: NaiveDate,
    end: NaiveDate,
    events: &[Event],
    detailed_activities: &[Activity],
    summaries_by_id: &HashMap<String, u8>,
    matched: &EventActivityMatches,
) -> Vec<RecentDayContext> {
    let activities_by_date = group_activities_by_date(detailed_activities);
    let events_by_date = group_events_by_date(events);

    date_range_inclusive(start, end)
        .into_iter()
        .map(|date| {
            let date_key = date.format("%Y-%m-%d").to_string();
            let day_activities = activities_by_date
                .get(&date_key)
                .cloned()
                .unwrap_or_default();
            let day_events = events_by_date.get(&date_key).cloned().unwrap_or_default();
            let planned_workouts = day_events
                .iter()
                .filter(|event| event.category == EventCategory::Workout)
                .map(|event| {
                    build_planned_workout(event, matched.event_to_activity.contains_key(&event.id))
                })
                .collect::<Vec<_>>();
            let special_days = day_events
                .iter()
                .filter(|event| is_special_event(event))
                .map(build_special_day)
                .collect::<Vec<_>>();
            let sick_note = infer_sick_note(&special_days);
            let workouts = day_activities
                .iter()
                .map(|activity| {
                    build_recent_workout(
                        activity,
                        matched.activity_to_event.get(&activity.id),
                        summaries_by_id,
                    )
                })
                .collect::<Vec<_>>();

            RecentDayContext {
                date: date_key,
                free_day: workouts.is_empty()
                    && planned_workouts.is_empty()
                    && special_days.is_empty(),
                sick_day: sick_note.is_some(),
                sick_note,
                workouts,
                planned_workouts,
                special_days,
            }
        })
        .collect()
}

fn build_upcoming_day_contexts(
    start: NaiveDate,
    end: NaiveDate,
    events: &[Event],
) -> Vec<UpcomingDayContext> {
    let events_by_date = group_events_by_date(events);

    date_range_inclusive(start, end)
        .into_iter()
        .map(|date| {
            let date_key = date.format("%Y-%m-%d").to_string();
            let day_events = events_by_date.get(&date_key).cloned().unwrap_or_default();
            let planned_workouts = day_events
                .iter()
                .filter(|event| event.category == EventCategory::Workout)
                .map(|event| build_planned_workout(event, false))
                .collect::<Vec<_>>();
            let special_days = day_events
                .iter()
                .filter(|event| is_special_event(event))
                .map(build_special_day)
                .collect::<Vec<_>>();

            UpcomingDayContext {
                date: date_key,
                free_day: planned_workouts.is_empty() && special_days.is_empty(),
                planned_workouts,
                special_days,
            }
        })
        .collect()
}

fn build_recent_workout(
    activity: &Activity,
    matched_event: Option<&Event>,
    summaries_by_id: &HashMap<String, u8>,
) -> RecentWorkoutContext {
    let power_values_5s = extract_and_average_stream(&activity.details.streams, "watts");
    let cadence_values_5s = extract_and_average_stream(&activity.details.streams, "cadence");
    let planned_workout = matched_event.map(|event| {
        let parsed = parse_workout_doc(event.workout_doc.as_deref(), activity.metrics.ftp_watts);

        PlannedWorkoutReference {
            event_id: event.id,
            start_date_local: event.start_date_local.clone(),
            name: event.name.clone(),
            category: event.category.as_str().to_string(),
            interval_blocks: build_planned_workout_blocks(
                &parsed.segments,
                activity.metrics.ftp_watts,
            ),
            raw_workout_doc: event.workout_doc.clone(),
            estimated_training_stress_score: parsed.summary.estimated_training_stress_score,
            estimated_intensity_factor: parsed.summary.estimated_intensity_factor,
            estimated_normalized_power_watts: parsed.summary.estimated_normalized_power_watts,
            completed: true,
        }
    });

    RecentWorkoutContext {
        activity_id: activity.id.clone(),
        start_date_local: activity.start_date_local.clone(),
        name: activity.name.clone(),
        activity_type: activity.activity_type.clone(),
        training_stress_score: activity.metrics.training_stress_score,
        efficiency_factor: activity.metrics.efficiency_factor,
        intensity_factor: activity.metrics.intensity_factor,
        normalized_power_watts: activity.metrics.normalized_power_watts,
        ftp_watts: activity.metrics.ftp_watts,
        rpe: summaries_by_id.get(&activity.id).copied(),
        variability_index: activity.metrics.variability_index,
        power_values_5s,
        cadence_values_5s,
        planned_workout,
    }
}

fn build_planned_workout(event: &Event, completed: bool) -> PlannedWorkoutContext {
    let parsed = parse_workout_doc(event.workout_doc.as_deref(), None);
    PlannedWorkoutContext {
        event_id: event.id,
        start_date_local: event.start_date_local.clone(),
        name: event.name.clone(),
        category: event.category.as_str().to_string(),
        interval_blocks: build_planned_workout_blocks(&parsed.segments, None),
        raw_workout_doc: event.workout_doc.clone(),
        estimated_training_stress_score: parsed.summary.estimated_training_stress_score,
        estimated_intensity_factor: parsed.summary.estimated_intensity_factor,
        estimated_normalized_power_watts: parsed.summary.estimated_normalized_power_watts,
        completed,
    }
}

fn build_special_day(event: &Event) -> SpecialDayContext {
    SpecialDayContext {
        event_id: event.id,
        start_date_local: event.start_date_local.clone(),
        name: event.name.clone(),
        category: event.category.as_str().to_string(),
        description: event.description.clone(),
    }
}

fn build_planned_workout_blocks(
    segments: &[WorkoutSegment],
    ftp_watts: Option<i32>,
) -> Vec<PlannedWorkoutBlockContext> {
    segments
        .iter()
        .map(|segment| PlannedWorkoutBlockContext {
            duration_seconds: segment.duration_seconds,
            min_percent_ftp: segment.min_target_percent_ftp,
            max_percent_ftp: segment.max_target_percent_ftp,
            min_target_watts: ftp_watts.and_then(|ftp| {
                segment
                    .min_target_percent_ftp
                    .map(|percent| (ftp as f64 * percent / 100.0).round() as i32)
            }),
            max_target_watts: ftp_watts.and_then(|ftp| {
                segment
                    .max_target_percent_ftp
                    .map(|percent| (ftp as f64 * percent / 100.0).round() as i32)
            }),
        })
        .collect()
}

fn infer_sick_note(special_days: &[SpecialDayContext]) -> Option<String> {
    special_days.iter().find_map(|special| {
        let mut text = String::new();
        if let Some(name) = &special.name {
            text.push_str(name);
            text.push(' ');
        }
        if let Some(description) = &special.description {
            text.push_str(description);
        }

        let normalized = text.trim().to_ascii_lowercase();
        if normalized.contains("sick")
            || normalized.contains("ill")
            || normalized.contains("unwell")
            || normalized.contains("fever")
            || normalized.contains("cold")
            || normalized.contains("flu")
        {
            Some(text.trim().to_string())
        } else {
            None
        }
    })
}

#[derive(Default)]
struct EventActivityMatches {
    event_to_activity: HashMap<i64, String>,
    activity_to_event: HashMap<String, Event>,
}

fn build_event_activity_matches(
    events: &[Event],
    activities: &[Activity],
    configured_ftp: Option<i32>,
) -> EventActivityMatches {
    let mut scored_matches = Vec::new();
    for event in events
        .iter()
        .filter(|event| event.category == EventCategory::Workout)
    {
        let event_date = date_key(&event.start_date_local);
        let same_day = activities
            .iter()
            .filter(|activity| date_key(&activity.start_date_local) == event_date)
            .collect::<Vec<_>>();
        let effective_ftp = configured_ftp.or_else(|| {
            same_day
                .iter()
                .find_map(|activity| activity.metrics.ftp_watts)
        });
        let parsed = parse_workout_doc(event.workout_doc.as_deref(), effective_ftp);

        for activity in same_day {
            if let Some(candidate) =
                find_best_activity_match(&parsed, std::slice::from_ref(activity), effective_ftp)
            {
                scored_matches.push((candidate.compliance_score, event.clone(), activity.clone()));
            }
        }
    }

    scored_matches.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut used_event_ids = BTreeSet::new();
    let mut used_activity_ids = BTreeSet::new();
    let mut matches = EventActivityMatches::default();
    for (_score, event, activity) in scored_matches {
        if used_event_ids.contains(&event.id) || used_activity_ids.contains(&activity.id) {
            continue;
        }
        used_event_ids.insert(event.id);
        used_activity_ids.insert(activity.id.clone());
        matches
            .event_to_activity
            .insert(event.id, activity.id.clone());
        matches.activity_to_event.insert(activity.id, event);
    }

    matches
}

fn group_activities_by_date(activities: &[Activity]) -> BTreeMap<String, Vec<Activity>> {
    let mut grouped = BTreeMap::new();
    for activity in activities {
        grouped
            .entry(date_key(&activity.start_date_local))
            .or_insert_with(Vec::new)
            .push(activity.clone());
    }
    grouped
}

fn group_events_by_date(events: &[Event]) -> BTreeMap<String, Vec<Event>> {
    let mut grouped = BTreeMap::new();
    for event in events {
        grouped
            .entry(date_key(&event.start_date_local))
            .or_insert_with(Vec::new)
            .push(event.clone());
    }
    grouped
}

fn infer_focus_kind(
    workout_id: &str,
    recent_days: &[RecentDayContext],
    upcoming_days: &[UpcomingDayContext],
) -> String {
    if recent_days
        .iter()
        .flat_map(|day| &day.workouts)
        .any(|workout| workout.activity_id == workout_id)
    {
        return "activity".to_string();
    }
    if recent_days
        .iter()
        .flat_map(|day| &day.planned_workouts)
        .any(|planned| planned.event_id.to_string() == workout_id)
        || upcoming_days
            .iter()
            .flat_map(|day| &day.planned_workouts)
            .any(|planned| planned.event_id.to_string() == workout_id)
    {
        return "event".to_string();
    }
    "summary".to_string()
}

fn extract_and_average_stream(streams: &[ActivityStream], stream_type: &str) -> Vec<i32> {
    let values = streams
        .iter()
        .find(|stream| stream.stream_type.eq_ignore_ascii_case(stream_type))
        .and_then(|stream| stream.data.as_ref())
        .map(extract_numeric_values)
        .unwrap_or_default();

    let chunks = values
        .chunks(STREAM_BUCKET_SIZE)
        .map(|chunk| (chunk.iter().sum::<i32>() as f64 / chunk.len() as f64).round() as i32)
        .collect::<Vec<_>>();

    compress_stream_chunks(chunks)
}

fn compress_stream_chunks(chunks: Vec<i32>) -> Vec<i32> {
    if chunks.len() <= MAX_CHUNKS_PER_WORKOUT {
        return chunks;
    }

    let recent_count = MAX_CHUNKS_PER_WORKOUT / 2;
    let summary_count = MAX_CHUNKS_PER_WORKOUT - recent_count;
    let older_count = chunks.len() - recent_count;
    let group_size = older_count.div_ceil(summary_count);
    let summarized = chunks[..older_count]
        .chunks(group_size)
        .map(|group| (group.iter().sum::<i32>() as f64 / group.len() as f64).round() as i32);

    summarized
        .chain(chunks[older_count..].iter().copied())
        .collect()
}

fn activity_has_required_detail(activity: &Activity) -> bool {
    !activity.details.streams.is_empty()
}

fn extract_numeric_values(value: &serde_json::Value) -> Vec<i32> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_i64())
                .filter_map(|item| i32::try_from(item).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn intervals_status_message(error: &IntervalsError) -> String {
    match error {
        IntervalsError::CredentialsNotConfigured => "credentials_not_configured".to_string(),
        IntervalsError::Unauthenticated => "unauthenticated".to_string(),
        IntervalsError::ApiError(_) => "api_error".to_string(),
        IntervalsError::ConnectionError(_) => "connection_error".to_string(),
        IntervalsError::NotFound => "not_found".to_string(),
        IntervalsError::Internal(_) => "internal_error".to_string(),
    }
}

fn is_special_event(event: &Event) -> bool {
    !matches!(event.category, EventCategory::Workout)
}

fn build_daily_tss_map(
    start: NaiveDate,
    end: NaiveDate,
    activities: &[Activity],
) -> BTreeMap<NaiveDate, i32> {
    let mut values = date_range_inclusive(start, end)
        .into_iter()
        .map(|date| (date, 0))
        .collect::<BTreeMap<_, _>>();

    for activity in activities {
        if let Some(tss) = activity.metrics.training_stress_score {
            let date = activity_date(activity);
            if let Some(current) = values.get_mut(&date) {
                *current += tss;
            }
        }
    }

    values
}

fn build_load_trend(
    values: &BTreeMap<NaiveDate, i32>,
    trend_days: usize,
    ctl_window_days: usize,
) -> Vec<HistoricalLoadTrendPoint> {
    let entries = values
        .iter()
        .map(|(date, tss)| (*date, *tss))
        .collect::<Vec<_>>();
    let start_index = entries.len().saturating_sub(trend_days);
    entries
        .iter()
        .enumerate()
        .skip(start_index)
        .map(|(index, (_, daily_tss))| {
            build_load_trend_point(&entries, index, ctl_window_days, 1, *daily_tss)
        })
        .collect()
}

fn build_recent_interval_blocks_by_activity_id(
    detailed_activities: &[Activity],
    matched: &EventActivityMatches,
) -> HashMap<String, Vec<PlannedWorkoutBlockContext>> {
    detailed_activities
        .iter()
        .filter_map(|activity| {
            matched.activity_to_event.get(&activity.id).map(|event| {
                let parsed =
                    parse_workout_doc(event.workout_doc.as_deref(), activity.metrics.ftp_watts);
                (
                    activity.id.clone(),
                    build_planned_workout_blocks(&parsed.segments, activity.metrics.ftp_watts),
                )
            })
        })
        .collect()
}

fn build_load_trend_point(
    entries: &[(NaiveDate, i32)],
    end_index: usize,
    ctl_window_days: usize,
    sample_days: u8,
    period_tss: i32,
) -> HistoricalLoadTrendPoint {
    let ctl = ewma_at_index(entries, end_index, ctl_window_days);
    let atl = ewma_at_index(entries, end_index, 7);

    HistoricalLoadTrendPoint {
        date: entries[end_index].0.format("%Y-%m-%d").to_string(),
        sample_days,
        period_tss,
        rolling_tss_7d: rolling_average_at_index(entries, end_index, 7),
        rolling_tss_28d: rolling_average_at_index(entries, end_index, 28),
        ctl,
        atl,
        tsb: match (ctl, atl) {
            (Some(ctl), Some(atl)) => Some(round_to_2(ctl - atl)),
            _ => None,
        },
    }
}

fn ewma_latest(values: &BTreeMap<NaiveDate, i32>, time_constant_days: usize) -> Option<f64> {
    let entries = values
        .iter()
        .map(|(date, tss)| (*date, *tss))
        .collect::<Vec<_>>();
    entries
        .len()
        .checked_sub(1)
        .and_then(|end_index| ewma_at_index(&entries, end_index, time_constant_days))
}

fn ewma_at_index(
    values: &[(NaiveDate, i32)],
    end_index: usize,
    time_constant_days: usize,
) -> Option<f64> {
    if values.is_empty() || time_constant_days == 0 || end_index >= values.len() {
        return None;
    }

    let alpha = 1.0 / time_constant_days as f64;
    let mut current = 0.0;
    for (_, tss) in values.iter().take(end_index + 1) {
        current += (*tss as f64 - current) * alpha;
    }

    Some(round_to_2(current))
}

fn rolling_average_at_index(
    values: &[(NaiveDate, i32)],
    end_index: usize,
    window_days: usize,
) -> Option<f64> {
    if values.is_empty() || end_index + 1 < window_days {
        return None;
    }

    let start_index = end_index + 1 - window_days;
    let total = values[start_index..=end_index]
        .iter()
        .map(|(_, value)| *value)
        .sum::<i32>();

    Some(round_to_2(total as f64 / window_days as f64))
}

fn average_recent_tss(values: &BTreeMap<NaiveDate, i32>, window_days: usize) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let total = values.values().rev().take(window_days).sum::<i32>();
    let count = values.values().rev().take(window_days).count();
    if count == 0 {
        None
    } else {
        Some(round_to_2(total as f64 / count as f64))
    }
}

fn average_metric(values: impl Iterator<Item = f64>) -> Option<f64> {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        None
    } else {
        Some(round_to_2(
            collected.iter().sum::<f64>() / collected.len() as f64,
        ))
    }
}

fn recent_slice(activities: &[Activity], start: NaiveDate) -> Vec<&Activity> {
    activities
        .iter()
        .filter(|activity| activity_date(activity) >= start)
        .collect()
}

fn date_range_inclusive(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut current = start;
    while current <= end {
        dates.push(current);
        current += Duration::days(1);
    }
    dates
}

fn activity_date(activity: &Activity) -> NaiveDate {
    parse_date(&date_key(&activity.start_date_local))
}

fn event_date(event: &Event) -> NaiveDate {
    parse_date(&date_key(&event.start_date_local))
}

fn date_key(value: &str) -> String {
    value.get(..10).unwrap_or(value).to_string()
}

fn parse_date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::DateTime::UNIX_EPOCH.date_naive())
}

fn epoch_seconds_to_date(epoch_seconds: i64) -> NaiveDate {
    chrono::DateTime::from_timestamp(epoch_seconds, 0)
        .map(|time| time.date_naive())
        .unwrap_or_else(|| Utc::now().date_naive())
}

fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::{
        identity::Clock,
        intervals::{
            ActivityDetails, ActivityMetrics, ActivityStream, EventCategory, IntervalsError,
        },
        settings::{
            AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError,
            UserSettings,
        },
        workout_summary::{ConversationMessage, MessageRole, WorkoutSummary, WorkoutSummaryError},
    };

    use super::*;

    #[derive(Clone)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_seconds(&self) -> i64 {
            1_775_174_400
        }
    }

    #[derive(Clone)]
    struct TestSettingsService;

    impl UserSettingsUseCases for TestSettingsService {
        fn get_settings(
            &self,
            _user_id: &str,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            Box::pin(async move {
                let mut settings = UserSettings::new_defaults("user-1".to_string(), 1);
                settings.cycling = CyclingSettings {
                    full_name: Some("Alex".to_string()),
                    ftp_watts: Some(300),
                    athlete_prompt: Some("prefers concise coaching".to_string()),
                    ..CyclingSettings::default()
                };
                Ok(settings)
            })
        }

        fn update_ai_agents(
            &self,
            _user_id: &str,
            _ai_agents: AiAgentsConfig,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            unreachable!()
        }
        fn update_intervals(
            &self,
            _user_id: &str,
            _intervals: IntervalsConfig,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            unreachable!()
        }
        fn update_options(
            &self,
            _user_id: &str,
            _options: AnalysisOptions,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            unreachable!()
        }
        fn update_cycling(
            &self,
            _user_id: &str,
            _cycling: CyclingSettings,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            unreachable!()
        }
    }

    #[derive(Clone)]
    struct TestIntervalsService;

    impl IntervalsUseCases for TestIntervalsService {
        fn list_events(
            &self,
            _user_id: &str,
            _range: &DateRange,
        ) -> crate::domain::intervals::BoxFuture<Result<Vec<Event>, IntervalsError>> {
            Box::pin(async move {
                Ok(vec![
                    Event {
                        id: 101,
                        start_date_local: "2026-04-03T07:00:00".to_string(),
                        name: Some("Sweet Spot".to_string()),
                        category: EventCategory::Workout,
                        description: None,
                        indoor: false,
                        color: None,
                        workout_doc: Some("- 2x10min 90-95%".to_string()),
                    },
                    Event {
                        id: 202,
                        start_date_local: "2026-04-02T09:00:00".to_string(),
                        name: Some("Sick day".to_string()),
                        category: EventCategory::Note,
                        description: Some("Felt unwell with sore throat".to_string()),
                        indoor: false,
                        color: None,
                        workout_doc: None,
                    },
                ])
            })
        }

        fn get_event(
            &self,
            _user_id: &str,
            _event_id: i64,
        ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
            unreachable!()
        }
        fn create_event(
            &self,
            _user_id: &str,
            _event: crate::domain::intervals::CreateEvent,
        ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
            unreachable!()
        }
        fn update_event(
            &self,
            _user_id: &str,
            _event_id: i64,
            _event: crate::domain::intervals::UpdateEvent,
        ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
            unreachable!()
        }
        fn delete_event(
            &self,
            _user_id: &str,
            _event_id: i64,
        ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
            unreachable!()
        }
        fn download_fit(
            &self,
            _user_id: &str,
            _event_id: i64,
        ) -> crate::domain::intervals::BoxFuture<Result<Vec<u8>, IntervalsError>> {
            unreachable!()
        }
        fn list_activities(
            &self,
            _user_id: &str,
            _range: &DateRange,
        ) -> crate::domain::intervals::BoxFuture<Result<Vec<Activity>, IntervalsError>> {
            Box::pin(async move {
                Ok(vec![Activity {
                    id: "ride-1".to_string(),
                    athlete_id: None,
                    start_date_local: "2026-04-03T08:00:00".to_string(),
                    start_date: None,
                    name: Some("Sweet Spot".to_string()),
                    description: None,
                    activity_type: Some("Ride".to_string()),
                    source: None,
                    external_id: None,
                    device_name: None,
                    distance_meters: None,
                    moving_time_seconds: Some(3600),
                    elapsed_time_seconds: Some(3600),
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
                    has_heart_rate: false,
                    stream_types: vec!["watts".to_string(), "cadence".to_string()],
                    tags: Vec::new(),
                    metrics: ActivityMetrics {
                        training_stress_score: Some(80),
                        normalized_power_watts: Some(250),
                        intensity_factor: Some(0.83),
                        efficiency_factor: Some(1.2),
                        variability_index: Some(1.05),
                        average_power_watts: Some(238),
                        ftp_watts: Some(300),
                        total_work_joules: None,
                        calories: None,
                        trimp: None,
                        power_load: None,
                        heart_rate_load: None,
                        pace_load: None,
                        strain_score: None,
                    },
                    details: ActivityDetails {
                        intervals: vec![
                            crate::domain::intervals::ActivityInterval {
                                id: Some(1),
                                label: Some("Work 1".to_string()),
                                interval_type: Some("WORK".to_string()),
                                group_id: Some("g1".to_string()),
                                start_index: Some(600),
                                end_index: Some(1200),
                                start_time_seconds: Some(600),
                                end_time_seconds: Some(1200),
                                moving_time_seconds: Some(600),
                                elapsed_time_seconds: Some(600),
                                distance_meters: None,
                                average_power_watts: Some(278),
                                normalized_power_watts: Some(280),
                                training_stress_score: Some(20.0),
                                average_heart_rate_bpm: None,
                                average_cadence_rpm: None,
                                average_speed_mps: None,
                                average_stride_meters: None,
                                zone: Some(4),
                            },
                            crate::domain::intervals::ActivityInterval {
                                id: Some(2),
                                label: Some("Work 2".to_string()),
                                interval_type: Some("WORK".to_string()),
                                group_id: Some("g1".to_string()),
                                start_index: Some(1500),
                                end_index: Some(2100),
                                start_time_seconds: Some(1500),
                                end_time_seconds: Some(2100),
                                moving_time_seconds: Some(600),
                                elapsed_time_seconds: Some(600),
                                distance_meters: None,
                                average_power_watts: Some(279),
                                normalized_power_watts: Some(281),
                                training_stress_score: Some(20.0),
                                average_heart_rate_bpm: None,
                                average_cadence_rpm: None,
                                average_speed_mps: None,
                                average_stride_meters: None,
                                zone: Some(4),
                            },
                        ],
                        interval_groups: Vec::new(),
                        streams: vec![
                            ActivityStream {
                                stream_type: "watts".to_string(),
                                name: None,
                                data: Some(serde_json::json!([200, 220, 240, 260, 280])),
                                data2: None,
                                value_type_is_array: false,
                                custom: false,
                                all_null: false,
                            },
                            ActivityStream {
                                stream_type: "cadence".to_string(),
                                name: None,
                                data: Some(serde_json::json!([80, 82, 84, 86, 88])),
                                data2: None,
                                value_type_is_array: false,
                                custom: false,
                                all_null: false,
                            },
                        ],
                        interval_summary: Vec::new(),
                        skyline_chart: Vec::new(),
                        power_zone_times: Vec::new(),
                        heart_rate_zone_times: Vec::new(),
                        pace_zone_times: Vec::new(),
                        gap_zone_times: Vec::new(),
                    },
                    details_unavailable_reason: None,
                }])
            })
        }

        fn get_activity(
            &self,
            _user_id: &str,
            _activity_id: &str,
        ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
            Box::pin(async move {
                Ok(Activity {
                    id: "ride-1".to_string(),
                    athlete_id: None,
                    start_date_local: "2026-04-03T08:00:00".to_string(),
                    start_date: None,
                    name: Some("Sweet Spot".to_string()),
                    description: None,
                    activity_type: Some("Ride".to_string()),
                    source: None,
                    external_id: None,
                    device_name: None,
                    distance_meters: None,
                    moving_time_seconds: Some(3600),
                    elapsed_time_seconds: Some(3600),
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
                    has_heart_rate: false,
                    stream_types: vec!["watts".to_string(), "cadence".to_string()],
                    tags: Vec::new(),
                    metrics: ActivityMetrics {
                        training_stress_score: Some(80),
                        normalized_power_watts: Some(250),
                        intensity_factor: Some(0.83),
                        efficiency_factor: Some(1.2),
                        variability_index: Some(1.05),
                        average_power_watts: Some(238),
                        ftp_watts: Some(300),
                        total_work_joules: None,
                        calories: None,
                        trimp: None,
                        power_load: None,
                        heart_rate_load: None,
                        pace_load: None,
                        strain_score: None,
                    },
                    details: ActivityDetails {
                        intervals: vec![
                            crate::domain::intervals::ActivityInterval {
                                id: Some(1),
                                label: Some("Work 1".to_string()),
                                interval_type: Some("WORK".to_string()),
                                group_id: Some("g1".to_string()),
                                start_index: Some(600),
                                end_index: Some(1200),
                                start_time_seconds: Some(600),
                                end_time_seconds: Some(1200),
                                moving_time_seconds: Some(600),
                                elapsed_time_seconds: Some(600),
                                distance_meters: None,
                                average_power_watts: Some(278),
                                normalized_power_watts: Some(280),
                                training_stress_score: Some(20.0),
                                average_heart_rate_bpm: None,
                                average_cadence_rpm: None,
                                average_speed_mps: None,
                                average_stride_meters: None,
                                zone: Some(4),
                            },
                            crate::domain::intervals::ActivityInterval {
                                id: Some(2),
                                label: Some("Work 2".to_string()),
                                interval_type: Some("WORK".to_string()),
                                group_id: Some("g1".to_string()),
                                start_index: Some(1500),
                                end_index: Some(2100),
                                start_time_seconds: Some(1500),
                                end_time_seconds: Some(2100),
                                moving_time_seconds: Some(600),
                                elapsed_time_seconds: Some(600),
                                distance_meters: None,
                                average_power_watts: Some(279),
                                normalized_power_watts: Some(281),
                                training_stress_score: Some(20.0),
                                average_heart_rate_bpm: None,
                                average_cadence_rpm: None,
                                average_speed_mps: None,
                                average_stride_meters: None,
                                zone: Some(4),
                            },
                        ],
                        interval_groups: Vec::new(),
                        streams: vec![
                            ActivityStream {
                                stream_type: "watts".to_string(),
                                name: None,
                                data: Some(serde_json::json!([200, 220, 240, 260, 280])),
                                data2: None,
                                value_type_is_array: false,
                                custom: false,
                                all_null: false,
                            },
                            ActivityStream {
                                stream_type: "cadence".to_string(),
                                name: None,
                                data: Some(serde_json::json!([80, 82, 84, 86, 88])),
                                data2: None,
                                value_type_is_array: false,
                                custom: false,
                                all_null: false,
                            },
                        ],
                        interval_summary: Vec::new(),
                        skyline_chart: Vec::new(),
                        power_zone_times: Vec::new(),
                        heart_rate_zone_times: Vec::new(),
                        pace_zone_times: Vec::new(),
                        gap_zone_times: Vec::new(),
                    },
                    details_unavailable_reason: None,
                })
            })
        }
        fn upload_activity(
            &self,
            _user_id: &str,
            _upload: crate::domain::intervals::UploadActivity,
        ) -> crate::domain::intervals::BoxFuture<
            Result<crate::domain::intervals::UploadedActivities, IntervalsError>,
        > {
            unreachable!()
        }
        fn update_activity(
            &self,
            _user_id: &str,
            _activity_id: &str,
            _activity: crate::domain::intervals::UpdateActivity,
        ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
            unreachable!()
        }
        fn delete_activity(
            &self,
            _user_id: &str,
            _activity_id: &str,
        ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
            unreachable!()
        }
    }

    #[derive(Clone)]
    struct TestWorkoutSummaryRepository;

    impl WorkoutSummaryRepository for TestWorkoutSummaryRepository {
        fn find_by_user_id_and_workout_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<Option<WorkoutSummary>, WorkoutSummaryError>,
        > {
            Box::pin(async { Ok(None) })
        }

        fn find_by_user_id_and_workout_ids(
            &self,
            _user_id: &str,
            workout_ids: Vec<String>,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<Vec<WorkoutSummary>, WorkoutSummaryError>,
        > {
            Box::pin(async move {
                Ok(workout_ids
                    .into_iter()
                    .map(|id| WorkoutSummary {
                        id: format!("summary-{id}"),
                        user_id: "user-1".to_string(),
                        workout_id: id,
                        rpe: Some(7),
                        messages: vec![ConversationMessage {
                            id: "message-1".to_string(),
                            role: MessageRole::User,
                            content: "felt controlled".to_string(),
                            created_at_epoch_seconds: 1,
                        }],
                        saved_at_epoch_seconds: None,
                        created_at_epoch_seconds: 1,
                        updated_at_epoch_seconds: 1,
                    })
                    .collect())
            })
        }

        fn create(
            &self,
            _summary: WorkoutSummary,
        ) -> crate::domain::workout_summary::BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>
        {
            unreachable!()
        }

        fn update_rpe(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _rpe: u8,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
            unreachable!()
        }

        fn append_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _message: ConversationMessage,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
            unreachable!()
        }

        fn set_saved_state(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _saved_at_epoch_seconds: Option<i64>,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
            unreachable!()
        }

        fn find_message_by_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _message_id: &str,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<Option<ConversationMessage>, WorkoutSummaryError>,
        > {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn builder_renders_recent_and_historical_context() {
        let builder = DefaultTrainingContextBuilder::new(
            Arc::new(TestSettingsService),
            Arc::new(TestIntervalsService),
            Arc::new(TestWorkoutSummaryRepository),
            FixedClock,
        );

        let result = builder.build("user-1", "ride-1").await.unwrap();

        assert_eq!(result.context.focus_kind, "activity");
        assert_eq!(result.context.intervals_status.activities, "ok");
        assert_eq!(result.context.intervals_status.events, "ok");
        assert_eq!(result.context.recent_days.len(), 14);
        assert_eq!(result.context.history.load_trend.len(), 42);
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .first()
                .map(|point| point.sample_days),
            Some(1)
        );
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .first()
                .map(|point| point.date.as_str()),
            Some("2026-02-21")
        );
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .last()
                .map(|point| point.sample_days),
            Some(1)
        );
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .last()
                .map(|point| point.period_tss),
            Some(80)
        );
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .last()
                .and_then(|point| point.rolling_tss_7d),
            Some(11.43)
        );
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .last()
                .and_then(|point| point.rolling_tss_28d),
            Some(2.86)
        );
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .last()
                .and_then(|point| point.ctl),
            Some(1.9)
        );
        let recent_day = result
            .context
            .recent_days
            .iter()
            .find(|day| day.date == "2026-04-03")
            .expect("recent day should exist");
        assert_eq!(recent_day.workouts.len(), 1);
        assert!(!recent_day.sick_day);
        assert_eq!(recent_day.workouts[0].rpe, Some(7));
        assert_eq!(recent_day.workouts[0].power_values_5s, vec![240]);
        assert_eq!(
            recent_day.workouts[0]
                .planned_workout
                .as_ref()
                .map(|planned| planned
                    .interval_blocks
                    .iter()
                    .map(|block| block.duration_seconds)
                    .sum::<i32>()),
            Some(1200)
        );
        assert_eq!(
            recent_day.workouts[0]
                .planned_workout
                .as_ref()
                .map(|planned| planned.interval_blocks.len()),
            Some(2)
        );
        assert_eq!(
            recent_day.workouts[0]
                .planned_workout
                .as_ref()
                .and_then(|planned| planned.interval_blocks.first())
                .and_then(|block| block.min_target_watts),
            Some(270)
        );
        let sick_day = result
            .context
            .recent_days
            .iter()
            .find(|day| day.date == "2026-04-02")
            .expect("sick day should exist");
        assert!(sick_day.sick_day);
        assert_eq!(
            sick_day.sick_note.as_deref(),
            Some("Sick day Felt unwell with sore throat")
        );
        assert!(result
            .rendered
            .stable_context
            .contains("prefers concise coaching"));
        assert!(result.rendered.stable_context.contains("\"lt\":["));
        assert!(result.rendered.stable_context.contains("\"days\":1"));
        assert!(result.rendered.stable_context.contains("\"bl\":["));
        assert!(result.rendered.volatile_context.contains("\"ride-1\""));
    }

    #[tokio::test]
    async fn build_athlete_summary_context_uses_explicit_summary_focus() {
        let builder = DefaultTrainingContextBuilder::new(
            Arc::new(TestSettingsService),
            Arc::new(TestIntervalsService),
            Arc::new(TestWorkoutSummaryRepository),
            FixedClock,
        );

        let result = builder
            .build_athlete_summary_context("user-1")
            .await
            .unwrap();

        assert_eq!(result.context.focus_kind, "summary");
        assert_eq!(result.context.focus_workout_id, None);
        assert!(result
            .rendered
            .volatile_context
            .contains("\"k\":\"summary\""));
        assert!(result
            .rendered
            .volatile_context
            .contains("\"fx\":{\"k\":\"summary\"}"));
    }

    #[test]
    fn build_daily_tss_map_includes_zero_load_rest_days() {
        let start = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 4, 4).unwrap();
        let activities = vec![Activity {
            id: "ride-1".to_string(),
            athlete_id: None,
            start_date_local: "2026-04-03T08:00:00".to_string(),
            start_date: None,
            name: None,
            description: None,
            activity_type: None,
            source: None,
            external_id: None,
            device_name: None,
            distance_meters: None,
            moving_time_seconds: None,
            elapsed_time_seconds: None,
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
            has_heart_rate: false,
            stream_types: Vec::new(),
            tags: Vec::new(),
            metrics: ActivityMetrics {
                training_stress_score: Some(80),
                normalized_power_watts: None,
                intensity_factor: None,
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: None,
                ftp_watts: None,
                total_work_joules: None,
                calories: None,
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            details: ActivityDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: Vec::new(),
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            details_unavailable_reason: None,
        }];

        let daily_tss = build_daily_tss_map(start, end, &activities);

        assert_eq!(daily_tss.len(), 4);
        assert_eq!(
            daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
            Some(&0)
        );
        assert_eq!(
            daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 2).unwrap()),
            Some(&0)
        );
        assert_eq!(
            daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 3).unwrap()),
            Some(&80)
        );
        assert_eq!(
            daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 4).unwrap()),
            Some(&0)
        );
    }

    #[test]
    fn build_load_trend_uses_exponential_ctl_and_atl_not_simple_rolling_averages() {
        let start = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let values = [100, 0, 100, 0, 100, 0, 100]
            .into_iter()
            .enumerate()
            .map(|(index, tss)| (start + Duration::days(index as i64), tss))
            .collect::<BTreeMap<_, _>>();

        let load_trend = build_load_trend(&values, 7, 42);
        let last = load_trend.last().unwrap();

        assert_eq!(last.rolling_tss_7d, Some(57.14));
        assert_eq!(last.ctl, Some(8.87));
        assert_eq!(last.atl, Some(38.16));
        assert_eq!(last.tsb, Some(-29.29));
    }

    #[tokio::test]
    async fn builder_requests_longer_history_warmup_for_load_seed() {
        let builder = DefaultTrainingContextBuilder::new(
            Arc::new(TestSettingsService),
            Arc::new(TestIntervalsService),
            Arc::new(TestWorkoutSummaryRepository),
            FixedClock,
        );

        let result = builder.build("user-1", "ride-1").await.unwrap();

        assert_eq!(result.context.history.window_start, "2025-06-20");
        assert_eq!(
            result
                .context
                .history
                .load_trend
                .first()
                .map(|point| point.date.as_str()),
            Some("2026-02-21")
        );
    }
}
