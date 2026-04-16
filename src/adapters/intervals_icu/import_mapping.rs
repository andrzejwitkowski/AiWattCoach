use sha2::{Digest, Sha256};

use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    external_sync::{
        ExternalCompletedWorkoutImport, ExternalImportCommand, ExternalPlannedWorkoutImport,
        ExternalProvider, ExternalRaceImport, ExternalSpecialDayImport,
    },
    identity::IdGenerator,
    intervals::{self, Activity, Event, EventCategory},
    planned_workouts::PlannedWorkout,
    races::{Race, RaceDiscipline, RacePriority},
    special_days::{SpecialDay, SpecialDayKind},
};

pub fn map_event_to_import_command(
    user_id: &str,
    event: &Event,
    _ids: &impl IdGenerator,
) -> Result<Option<ExternalImportCommand>, intervals::PlannedWorkoutParseError> {
    match event.category {
        EventCategory::Workout => map_workout_event_import(user_id, event).map(Some),
        EventCategory::Race
        | EventCategory::RaceA
        | EventCategory::RaceB
        | EventCategory::RaceC => Ok(Some(map_race_event_import(user_id, event))),
        EventCategory::Note
        | EventCategory::Target
        | EventCategory::Season
        | EventCategory::Other => Ok(Some(map_special_day_event_import(user_id, event))),
    }
}

fn map_workout_event_import(
    user_id: &str,
    event: &Event,
) -> Result<ExternalImportCommand, intervals::PlannedWorkoutParseError> {
    Ok(ExternalImportCommand::UpsertPlannedWorkout(
        ExternalPlannedWorkoutImport {
            provider: ExternalProvider::Intervals,
            external_id: event.id.to_string(),
            normalized_payload_hash: hash_event(event),
            workout: PlannedWorkout::new(
                format!("intervals-event:{}", event.id),
                user_id.to_string(),
                event_date(&event.start_date_local).to_string(),
                map_event_to_planned_workout_payload(event)?,
            )
            .with_event_metadata(
                event.name.clone(),
                event.description.clone(),
                event.event_type.clone(),
            ),
        },
    ))
}

fn map_race_event_import(user_id: &str, event: &Event) -> ExternalImportCommand {
    ExternalImportCommand::UpsertRace(ExternalRaceImport {
        provider: ExternalProvider::Intervals,
        external_id: event.id.to_string(),
        normalized_payload_hash: hash_event(event),
        race: Race {
            race_id: format!("intervals-race:{}", event.id),
            user_id: user_id.to_string(),
            date: event_date(&event.start_date_local).to_string(),
            name: event
                .name
                .clone()
                .unwrap_or_else(|| "Intervals race".to_string()),
            distance_meters: infer_race_distance_meters(event.description.as_deref()),
            discipline: infer_race_discipline(event.event_type.as_deref()),
            priority: map_race_priority(&event.category),
            result: None,
            created_at_epoch_seconds: 0,
            updated_at_epoch_seconds: 0,
        },
    })
}

fn map_special_day_event_import(user_id: &str, event: &Event) -> ExternalImportCommand {
    ExternalImportCommand::UpsertSpecialDay(ExternalSpecialDayImport {
        provider: ExternalProvider::Intervals,
        external_id: event.id.to_string(),
        normalized_payload_hash: hash_event(event),
        special_day: SpecialDay::new(
            format!("intervals-special-day:{}", event.id),
            user_id.to_string(),
            event_date(&event.start_date_local).to_string(),
            map_special_day_kind(&event.category),
            event.name.clone(),
            event.description.clone(),
        )
        .expect("intervals special day import should always produce canonical YYYY-MM-DD date"),
    })
}

pub fn map_activity_to_import_command(user_id: &str, activity: &Activity) -> ExternalImportCommand {
    let workout = map_activity_to_completed_workout(user_id, activity);
    ExternalImportCommand::UpsertCompletedWorkout(Box::new(ExternalCompletedWorkoutImport {
        provider: ExternalProvider::Intervals,
        external_id: activity.id.clone(),
        normalized_payload_hash: hash_activity(activity),
        marker_sources: vec![
            activity.external_id.clone(),
            activity.description.clone(),
            activity.name.clone(),
        ]
        .into_iter()
        .flatten()
        .collect(),
        workout,
    }))
}

fn map_activity_to_completed_workout(user_id: &str, activity: &Activity) -> CompletedWorkout {
    CompletedWorkout::new(
        format!("intervals-activity:{}", activity.id),
        user_id.to_string(),
        activity.start_date_local.clone(),
        Some(activity.id.clone()),
        None,
        activity.name.clone(),
        activity.description.clone(),
        activity.activity_type.clone(),
        activity.external_id.clone(),
        activity.trainer,
        activity
            .elapsed_time_seconds
            .or(activity.moving_time_seconds),
        activity.distance_meters,
        CompletedWorkoutMetrics {
            training_stress_score: activity.metrics.training_stress_score,
            normalized_power_watts: activity.metrics.normalized_power_watts,
            intensity_factor: activity.metrics.intensity_factor,
            efficiency_factor: activity.metrics.efficiency_factor,
            variability_index: activity.metrics.variability_index,
            average_power_watts: activity.metrics.average_power_watts,
            ftp_watts: activity.metrics.ftp_watts,
            total_work_joules: activity.metrics.total_work_joules,
            calories: activity.metrics.calories,
            trimp: activity.metrics.trimp,
            power_load: activity.metrics.power_load,
            heart_rate_load: activity.metrics.heart_rate_load,
            pace_load: activity.metrics.pace_load,
            strain_score: activity.metrics.strain_score,
        },
        CompletedWorkoutDetails {
            intervals: activity
                .details
                .intervals
                .iter()
                .cloned()
                .map(
                    |interval| crate::domain::completed_workouts::CompletedWorkoutInterval {
                        id: interval.id,
                        label: interval.label,
                        interval_type: interval.interval_type,
                        group_id: interval.group_id,
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
                    },
                )
                .collect(),
            interval_groups: activity
                .details
                .interval_groups
                .iter()
                .cloned()
                .map(
                    |group| crate::domain::completed_workouts::CompletedWorkoutIntervalGroup {
                        id: group.id,
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
                    },
                )
                .collect(),
            streams: activity
                .details
                .streams
                .iter()
                .cloned()
                .map(|stream| CompletedWorkoutStream {
                    stream_type: stream.stream_type,
                    name: stream.name,
                    primary_series: map_stream_series(stream.data),
                    secondary_series: map_stream_series(stream.data2),
                    value_type_is_array: stream.value_type_is_array,
                    custom: stream.custom,
                    all_null: stream.all_null,
                })
                .collect(),
            interval_summary: activity.details.interval_summary.clone(),
            skyline_chart: activity.details.skyline_chart.clone(),
            power_zone_times: activity
                .details
                .power_zone_times
                .iter()
                .cloned()
                .map(|zone| CompletedWorkoutZoneTime {
                    zone_id: zone.zone_id,
                    seconds: zone.seconds,
                })
                .collect(),
            heart_rate_zone_times: activity.details.heart_rate_zone_times.clone(),
            pace_zone_times: activity.details.pace_zone_times.clone(),
            gap_zone_times: activity.details.gap_zone_times.clone(),
        },
        activity.details_unavailable_reason.clone(),
    )
}

fn map_stream_series(value: Option<serde_json::Value>) -> Option<CompletedWorkoutSeries> {
    let value = value?;
    let serde_json::Value::Array(items) = value else {
        return None;
    };

    if items.iter().all(|item| item.as_i64().is_some()) {
        return Some(CompletedWorkoutSeries::Integers(
            items.into_iter().filter_map(|item| item.as_i64()).collect(),
        ));
    }

    if items.iter().all(|item| item.as_f64().is_some()) {
        return Some(CompletedWorkoutSeries::Floats(
            items.into_iter().filter_map(|item| item.as_f64()).collect(),
        ));
    }

    if items.iter().all(|item| item.as_bool().is_some()) {
        return Some(CompletedWorkoutSeries::Bools(
            items
                .into_iter()
                .filter_map(|item| item.as_bool())
                .collect(),
        ));
    }

    if items.iter().all(|item| item.as_str().is_some()) {
        return Some(CompletedWorkoutSeries::Strings(
            items
                .into_iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect(),
        ));
    }

    None
}

fn map_event_to_planned_workout_payload(
    event: &Event,
) -> Result<
    crate::domain::planned_workouts::PlannedWorkoutContent,
    intervals::PlannedWorkoutParseError,
> {
    let parsed = intervals::parse_planned_workout(event.structured_workout_text().unwrap_or(""))?;

    Ok(crate::domain::planned_workouts::PlannedWorkoutContent {
        lines: parsed
            .lines
            .into_iter()
            .map(map_planned_workout_line)
            .collect(),
    })
}

fn map_planned_workout_line(
    line: intervals::PlannedWorkoutLine,
) -> crate::domain::planned_workouts::PlannedWorkoutLine {
    match line {
        intervals::PlannedWorkoutLine::Text(text) => {
            crate::domain::planned_workouts::PlannedWorkoutLine::Text(
                crate::domain::planned_workouts::PlannedWorkoutText { text: text.text },
            )
        }
        intervals::PlannedWorkoutLine::Repeat(repeat) => {
            crate::domain::planned_workouts::PlannedWorkoutLine::Repeat(
                crate::domain::planned_workouts::PlannedWorkoutRepeat {
                    title: repeat.title,
                    count: repeat.count,
                },
            )
        }
        intervals::PlannedWorkoutLine::Step(step) => {
            crate::domain::planned_workouts::PlannedWorkoutLine::Step(
                crate::domain::planned_workouts::PlannedWorkoutStep {
                    duration_seconds: step.duration_seconds,
                    kind: match step.kind {
                        intervals::PlannedWorkoutStepKind::Steady => {
                            crate::domain::planned_workouts::PlannedWorkoutStepKind::Steady
                        }
                        intervals::PlannedWorkoutStepKind::Ramp => {
                            crate::domain::planned_workouts::PlannedWorkoutStepKind::Ramp
                        }
                    },
                    target: match step.target {
                        intervals::PlannedWorkoutTarget::PercentFtp { min, max } => {
                            crate::domain::planned_workouts::PlannedWorkoutTarget::PercentFtp {
                                min,
                                max,
                            }
                        }
                        intervals::PlannedWorkoutTarget::WattsRange { min, max } => {
                            crate::domain::planned_workouts::PlannedWorkoutTarget::WattsRange {
                                min,
                                max,
                            }
                        }
                    },
                },
            )
        }
    }
}

fn event_date(start_date_local: &str) -> &str {
    start_date_local.get(..10).unwrap_or(start_date_local)
}

fn map_race_priority(category: &EventCategory) -> RacePriority {
    match category {
        EventCategory::RaceA => RacePriority::A,
        EventCategory::RaceB => RacePriority::B,
        EventCategory::RaceC => RacePriority::C,
        EventCategory::Race => RacePriority::C,
        _ => RacePriority::C,
    }
}

fn infer_race_discipline(event_type: Option<&str>) -> RaceDiscipline {
    match event_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "mtb" | "mountain bike" => RaceDiscipline::Mtb,
        "gravel" => RaceDiscipline::Gravel,
        "cyclocross" | "cx" => RaceDiscipline::Cyclocross,
        "timetrial" | "time trial" | "tt" => RaceDiscipline::Timetrial,
        _ => RaceDiscipline::Road,
    }
}

fn infer_race_distance_meters(description: Option<&str>) -> i32 {
    description
        .and_then(parse_first_distance_meters)
        .unwrap_or(0)
}

fn parse_first_distance_meters(text: &str) -> Option<i32> {
    let lower = text.to_ascii_lowercase();
    let tokens = lower
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',' || ch == ';')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if let Some(distance) = parse_distance_token(token) {
            return Some(distance);
        }

        let Some(next_token) = tokens.get(index + 1) else {
            continue;
        };
        if let Some(distance) = parse_split_distance_tokens(token, next_token) {
            return Some(distance);
        }
    }

    None
}

fn parse_distance_token(token: &str) -> Option<i32> {
    if let Some(km) = token.strip_suffix("km") {
        return kilometers_to_meters(km.parse::<f64>().ok()?);
    }

    if let Some(meters) = token.strip_suffix('m') {
        let meters = meters.parse::<i32>().ok()?;
        return (meters > 0).then_some(meters);
    }

    None
}

fn parse_split_distance_tokens(value: &str, unit: &str) -> Option<i32> {
    match unit {
        "km" => kilometers_to_meters(value.parse::<f64>().ok()?),
        "m" => {
            let meters = value.parse::<i32>().ok()?;
            (meters > 0).then_some(meters)
        }
        _ => None,
    }
}

fn kilometers_to_meters(kilometers: f64) -> Option<i32> {
    let meters = (kilometers * 1000.0).round();
    if meters.is_finite() && meters > 0.0 && meters <= i32::MAX as f64 {
        Some(meters as i32)
    } else {
        None
    }
}

fn map_special_day_kind(category: &EventCategory) -> SpecialDayKind {
    match category {
        EventCategory::Note => SpecialDayKind::Note,
        EventCategory::Target | EventCategory::Season | EventCategory::Other => {
            SpecialDayKind::Other
        }
        _ => SpecialDayKind::Other,
    }
}

fn hash_event(event: &Event) -> String {
    let digest = Sha256::digest(format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        event.id,
        event.start_date_local,
        event.category.as_str(),
        event.event_type.as_deref().unwrap_or_default(),
        event.name.as_deref().unwrap_or_default(),
        event.description.as_deref().unwrap_or_default(),
        event.workout_doc.as_deref().unwrap_or_default(),
    ));
    format!("{digest:x}")
}

fn hash_activity(activity: &Activity) -> String {
    let workout = map_activity_to_completed_workout("hash-user", activity);
    let digest = Sha256::digest(format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        workout.completed_workout_id,
        workout.start_date_local,
        workout.planned_workout_id.as_deref().unwrap_or_default(),
        workout.name.as_deref().unwrap_or_default(),
        workout.description.as_deref().unwrap_or_default(),
        workout.activity_type.as_deref().unwrap_or_default(),
        workout.trainer,
        workout
            .duration_seconds
            .map(|value| value.to_string())
            .unwrap_or_default(),
        workout
            .distance_meters
            .map(|value| value.to_string())
            .unwrap_or_default(),
        serde_json::to_string(&workout.metrics)
            .expect("completed workout metrics should serialize into a stable payload hash"),
        serde_json::to_string(&workout.details)
            .expect("completed workout details should serialize into a stable payload hash"),
        activity.external_id.as_deref().unwrap_or_default(),
    ));
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        external_sync::ExternalImportCommand,
        identity::IdGenerator,
        intervals::{ActivityDetails, ActivityMetrics, ActivityStream, ActivityZoneTime},
        special_days::SpecialDayKind,
    };

    use super::{map_activity_to_import_command, map_event_to_import_command};

    #[derive(Clone, Default)]
    struct FixedIdGenerator;

    impl IdGenerator for FixedIdGenerator {
        fn new_id(&self, prefix: &str) -> String {
            format!("{prefix}-generated-1")
        }
    }

    #[test]
    fn maps_workout_event_into_planned_workout_import() {
        let command = map_event_to_import_command(
            "user-1",
            &crate::domain::intervals::Event {
                id: 144,
                start_date_local: "2026-05-10T00:00:00".to_string(),
                event_type: Some("Ride".to_string()),
                name: Some("Threshold Builder".to_string()),
                category: crate::domain::intervals::EventCategory::Workout,
                description: Some("Threshold Builder\n- 10m 90-95%".to_string()),
                indoor: false,
                color: None,
                workout_doc: None,
            },
            &FixedIdGenerator,
        )
        .unwrap()
        .expect("expected import command");

        let ExternalImportCommand::UpsertPlannedWorkout(command) = command else {
            panic!("expected planned workout import");
        };

        assert_eq!(command.external_id, "144");
        assert_eq!(command.workout.planned_workout_id, "intervals-event:144");
        assert_eq!(command.workout.date, "2026-05-10");
        assert_eq!(command.workout.name.as_deref(), Some("Threshold Builder"));
        assert_eq!(
            command.workout.description.as_deref(),
            Some("Threshold Builder\n- 10m 90-95%")
        );
        assert_eq!(command.workout.event_type.as_deref(), Some("Ride"));
        assert_eq!(command.workout.workout.lines.len(), 2);
    }

    #[test]
    fn maps_race_event_into_race_import() {
        let command = map_event_to_import_command(
            "user-1",
            &crate::domain::intervals::Event {
                id: 55,
                start_date_local: "2026-09-12T00:00:00".to_string(),
                event_type: Some("Gravel".to_string()),
                name: Some("Gravel Attack".to_string()),
                category: crate::domain::intervals::EventCategory::RaceB,
                description: Some("120km target race".to_string()),
                indoor: false,
                color: None,
                workout_doc: None,
            },
            &FixedIdGenerator,
        )
        .unwrap()
        .expect("expected import command");

        let ExternalImportCommand::UpsertRace(command) = command else {
            panic!("expected race import");
        };

        assert_eq!(command.external_id, "55");
        assert_eq!(command.race.race_id, "intervals-race:55");
        assert_eq!(command.race.name, "Gravel Attack");
        assert_eq!(command.race.distance_meters, 120_000);
        assert_eq!(command.race.priority, crate::domain::races::RacePriority::B);
        assert_eq!(
            command.race.discipline,
            crate::domain::races::RaceDiscipline::Gravel
        );
    }

    #[test]
    fn maps_race_event_distance_when_unit_is_split_by_space() {
        let command = map_event_to_import_command(
            "user-1",
            &crate::domain::intervals::Event {
                id: 56,
                start_date_local: "2026-09-13T00:00:00".to_string(),
                event_type: Some("Road".to_string()),
                name: Some("Big Day".to_string()),
                category: crate::domain::intervals::EventCategory::Race,
                description: Some("Target event over 120 km with a fast finish".to_string()),
                indoor: false,
                color: None,
                workout_doc: None,
            },
            &FixedIdGenerator,
        )
        .unwrap()
        .expect("expected import command");

        let ExternalImportCommand::UpsertRace(command) = command else {
            panic!("expected race import");
        };

        assert_eq!(command.race.distance_meters, 120_000);
    }

    #[test]
    fn maps_race_event_distance_when_first_distance_token_is_invalid() {
        let command = map_event_to_import_command(
            "user-1",
            &crate::domain::intervals::Event {
                id: 57,
                start_date_local: "2026-09-14T00:00:00".to_string(),
                event_type: Some("Road".to_string()),
                name: Some("Circuit".to_string()),
                category: crate::domain::intervals::EventCategory::Race,
                description: Some("xkm warmup then 500 m sprint finish".to_string()),
                indoor: false,
                color: None,
                workout_doc: None,
            },
            &FixedIdGenerator,
        )
        .unwrap()
        .expect("expected import command");

        let ExternalImportCommand::UpsertRace(command) = command else {
            panic!("expected race import");
        };

        assert_eq!(command.race.distance_meters, 500);
    }

    #[test]
    fn maps_special_event_into_special_day_import() {
        let command = map_event_to_import_command(
            "user-1",
            &crate::domain::intervals::Event {
                id: 88,
                start_date_local: "2026-06-01T00:00:00".to_string(),
                event_type: None,
                name: Some("Recovery Note".to_string()),
                category: crate::domain::intervals::EventCategory::Note,
                description: Some("Keep easy".to_string()),
                indoor: false,
                color: None,
                workout_doc: None,
            },
            &FixedIdGenerator,
        )
        .unwrap()
        .expect("expected import command");

        let ExternalImportCommand::UpsertSpecialDay(command) = command else {
            panic!("expected special day import");
        };

        assert_eq!(command.external_id, "88");
        assert_eq!(
            command.special_day.special_day_id,
            "intervals-special-day:88"
        );
        assert_eq!(command.special_day.date, "2026-06-01");
        assert_eq!(command.special_day.kind, SpecialDayKind::Note);
        assert_eq!(command.special_day.title.as_deref(), Some("Recovery Note"));
        assert_eq!(
            command.special_day.description.as_deref(),
            Some("Keep easy")
        );
    }

    #[test]
    fn maps_activity_into_completed_workout_import() {
        let command = map_activity_to_import_command("user-1", &sample_activity());

        let ExternalImportCommand::UpsertCompletedWorkout(command) = command else {
            panic!("expected completed workout import");
        };

        assert_eq!(command.external_id, "activity-1");
        assert_eq!(
            command.workout.completed_workout_id,
            "intervals-activity:activity-1"
        );
        assert_eq!(command.workout.start_date_local, "2026-05-11T08:00:00");
        assert_eq!(command.workout.name.as_deref(), Some("Threshold Ride"));
        assert_eq!(command.workout.description.as_deref(), Some("Strong day"));
        assert_eq!(command.workout.activity_type.as_deref(), Some("Ride"));
        assert!(command.workout.trainer);
        assert_eq!(command.workout.duration_seconds, Some(3700));
        assert_eq!(command.workout.distance_meters, Some(35_000.0));
        assert_eq!(command.workout.metrics.training_stress_score, Some(78));
        assert_eq!(command.workout.details.streams.len(), 1);
    }

    #[test]
    fn activity_hash_changes_when_persisted_details_change() {
        let base = sample_activity();
        let mut changed = sample_activity();
        changed.details.interval_summary = vec!["tempo".to_string(), "extra note".to_string()];

        let ExternalImportCommand::UpsertCompletedWorkout(base_command) =
            map_activity_to_import_command("user-1", &base)
        else {
            panic!("expected completed workout import");
        };
        let ExternalImportCommand::UpsertCompletedWorkout(changed_command) =
            map_activity_to_import_command("user-1", &changed)
        else {
            panic!("expected completed workout import");
        };

        assert_ne!(
            base_command.normalized_payload_hash,
            changed_command.normalized_payload_hash
        );
    }

    fn sample_activity() -> crate::domain::intervals::Activity {
        crate::domain::intervals::Activity {
            id: "activity-1".to_string(),
            athlete_id: None,
            start_date_local: "2026-05-11T08:00:00".to_string(),
            start_date: None,
            name: Some("Threshold Ride".to_string()),
            description: Some("Strong day".to_string()),
            activity_type: Some("Ride".to_string()),
            source: Some("intervals".to_string()),
            external_id: Some("external-1".to_string()),
            device_name: Some("Trainer".to_string()),
            distance_meters: Some(35_000.0),
            moving_time_seconds: Some(3600),
            elapsed_time_seconds: Some(3700),
            total_elevation_gain_meters: Some(400.0),
            total_elevation_loss_meters: Some(400.0),
            average_speed_mps: Some(9.7),
            max_speed_mps: Some(15.0),
            average_heart_rate_bpm: Some(150),
            max_heart_rate_bpm: Some(175),
            average_cadence_rpm: Some(88.0),
            trainer: true,
            commute: false,
            race: false,
            has_heart_rate: true,
            stream_types: vec!["watts".to_string()],
            tags: vec!["quality".to_string()],
            metrics: ActivityMetrics {
                training_stress_score: Some(78),
                normalized_power_watts: Some(245),
                intensity_factor: Some(0.83),
                efficiency_factor: None,
                variability_index: Some(1.04),
                average_power_watts: Some(221),
                ftp_watts: Some(295),
                total_work_joules: Some(750),
                calories: Some(900),
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            details: ActivityDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: vec![ActivityStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    data: Some(serde_json::json!([180, 240, 310])),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                }],
                interval_summary: vec!["tempo".to_string()],
                skyline_chart: Vec::new(),
                power_zone_times: vec![ActivityZoneTime {
                    zone_id: "z3".to_string(),
                    seconds: 1200,
                }],
                heart_rate_zone_times: vec![600],
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            details_unavailable_reason: None,
        }
    }
}
