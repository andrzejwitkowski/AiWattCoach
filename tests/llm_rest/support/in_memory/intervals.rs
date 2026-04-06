use super::*;

#[derive(Clone, Default)]
pub(crate) struct InMemoryIntervalsService {
    activities: Arc<Mutex<Vec<Activity>>>,
}

impl InMemoryIntervalsService {
    pub(crate) fn seed_activities(&self, activities: Vec<Activity>) {
        *self.activities.lock().unwrap() = activities;
    }
}

impl IntervalsUseCases for InMemoryIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> LlmBoxFuture<Result<Vec<Event>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> LlmBoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn create_event(
        &self,
        _user_id: &str,
        _event: aiwattcoach::domain::intervals::CreateEvent,
    ) -> LlmBoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn update_event(
        &self,
        _user_id: &str,
        _event_id: i64,
        _event: aiwattcoach::domain::intervals::UpdateEvent,
    ) -> LlmBoxFuture<Result<Event, IntervalsError>> {
        unreachable!()
    }

    fn delete_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> LlmBoxFuture<Result<(), IntervalsError>> {
        unreachable!()
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> LlmBoxFuture<Result<Vec<u8>, IntervalsError>> {
        unreachable!()
    }

    fn list_activities(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> LlmBoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let activities = self.activities.lock().unwrap().clone();
        Box::pin(async move { Ok(activities) })
    }

    fn get_activity(
        &self,
        _user_id: &str,
        activity_id: &str,
    ) -> LlmBoxFuture<Result<Activity, IntervalsError>> {
        let activities = self.activities.lock().unwrap().clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            activities
                .into_iter()
                .find(|activity| activity.id == activity_id)
                .ok_or(IntervalsError::NotFound)
        })
    }
}

pub(crate) fn sample_activity(activity_id: &str) -> Activity {
    Activity {
        id: activity_id.to_string(),
        athlete_id: None,
        start_date_local: format!("{}T08:00:00", Utc::now().format("%Y-%m-%d")),
        start_date: None,
        name: Some("Sweet Spot".to_string()),
        description: None,
        activity_type: Some("Ride".to_string()),
        source: None,
        external_id: None,
        device_name: None,
        distance_meters: None,
        moving_time_seconds: Some(1800),
        elapsed_time_seconds: Some(1800),
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
            training_stress_score: Some(55),
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
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![
                ActivityStream {
                    stream_type: "watts".to_string(),
                    name: None,
                    data: Some(serde_json::json!([300, 300, 300])),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                ActivityStream {
                    stream_type: "cadence".to_string(),
                    name: None,
                    data: Some(serde_json::json!([80, 82, 84])),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn get_activity_returns_requested_seeded_activity() {
        let service = InMemoryIntervalsService::default();
        service.seed_activities(vec![sample_activity("ride-1"), sample_activity("ride-2")]);

        let activity = service.get_activity("user-1", "ride-2").await.unwrap();

        assert_eq!(activity.id, "ride-2");
    }

    #[test]
    fn sample_activity_uses_current_date_for_recent_window() {
        let expected_date_prefix = Utc::now().format("%Y-%m-%d").to_string();
        let activity = sample_activity("ride-1");

        assert!(activity.start_date_local.starts_with(&expected_date_prefix));
    }
}
