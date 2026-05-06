use super::{
    compute_power_curve, BoxFuture, CompletedWorkout, CompletedWorkoutError,
    CompletedWorkoutPowerCurve, CompletedWorkoutRepository,
};

#[derive(Clone)]
pub struct PowerCurveCompletedWorkoutRepository<Repo>
where
    Repo: CompletedWorkoutRepository,
{
    inner: Repo,
}

impl<Repo> PowerCurveCompletedWorkoutRepository<Repo>
where
    Repo: CompletedWorkoutRepository,
{
    pub fn new(inner: Repo) -> Self {
        Self { inner }
    }
}

impl<Repo> CompletedWorkoutRepository for PowerCurveCompletedWorkoutRepository<Repo>
where
    Repo: CompletedWorkoutRepository,
{
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner
            .find_by_user_id_and_completed_workout_id(user_id, completed_workout_id)
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner
            .find_by_user_id_and_source_activity_id(user_id, source_activity_id)
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner.find_latest_by_user_id(user_id)
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner.list_by_user_id(user_id)
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner
            .list_by_user_id_and_date_range(user_id, oldest, newest)
    }

    fn upsert(
        &self,
        mut workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            if workout.power_curve_5s.is_none() && workout.details_unavailable_reason.is_none() {
                if let Ok(curve) = compute_power_curve(&workout, 5) {
                    workout.power_curve_5s = Some(curve);
                }
            }
            inner.upsert(workout).await
        })
    }

    fn set_power_curve_5s_if_missing(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        curve: CompletedWorkoutPowerCurve,
    ) -> BoxFuture<Result<(), CompletedWorkoutError>> {
        self.inner
            .set_power_curve_5s_if_missing(user_id, completed_workout_id, curve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::completed_workouts::{
        CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingRepo {
        upserted: Arc<Mutex<Vec<CompletedWorkout>>>,
    }

    impl CompletedWorkoutRepository for RecordingRepo {
        fn find_by_user_id_and_completed_workout_id(
            &self,
            _user_id: &str,
            _completed_workout_id: &str,
        ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
            Box::pin(async { Ok(None) })
        }

        fn find_by_user_id_and_source_activity_id(
            &self,
            _user_id: &str,
            _source_activity_id: &str,
        ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
            Box::pin(async { Ok(None) })
        }

        fn find_latest_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
            Box::pin(async { Ok(None) })
        }

        fn list_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn list_by_user_id_and_date_range(
            &self,
            _user_id: &str,
            _oldest: &str,
            _newest: &str,
        ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn upsert(
            &self,
            workout: CompletedWorkout,
        ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
            let upserted = self.upserted.clone();
            Box::pin(async move {
                upserted
                    .lock()
                    .expect("mutex poisoned")
                    .push(workout.clone());
                Ok(workout)
            })
        }
    }

    fn sample_workout_with_watts(watts: Vec<i64>) -> CompletedWorkout {
        CompletedWorkout::new(
            "cw-1".to_string(),
            "u1".to_string(),
            "2026-01-01T12:00:00".to_string(),
            None,
            None,
            Some("ride".to_string()),
            None,
            Some("Ride".to_string()),
            None,
            false,
            None,
            None,
            CompletedWorkoutMetrics::default(),
            CompletedWorkoutDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: vec![CompletedWorkoutStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    primary_series: Some(CompletedWorkoutSeries::Integers(watts)),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                }],
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            None,
        )
    }

    #[tokio::test]
    async fn decorator_computes_5s_curve_on_upsert() {
        let inner = RecordingRepo::default();
        let wrapped = PowerCurveCompletedWorkoutRepository::new(inner.clone());

        let workout =
            sample_workout_with_watts(vec![200, 220, 250, 240, 210, 190, 230, 260, 270, 255]);
        assert!(workout.power_curve_5s.is_none());

        let result = wrapped.upsert(workout).await.unwrap();
        assert!(result.power_curve_5s.is_some());
        let curve = result.power_curve_5s.unwrap();
        assert_eq!(curve.resolution_seconds, 5);
        assert_eq!(curve.source_samples, 10);
        assert_eq!(curve.valid_power_samples, 10);
        assert_eq!(curve.max_average_watts.len(), 2);

        let stored = inner.upserted.lock().expect("mutex poisoned");
        assert_eq!(stored.len(), 1);
        assert!(stored[0].power_curve_5s.is_some());
    }

    #[tokio::test]
    async fn decorator_leaves_curve_none_when_details_unavailable() {
        let inner = RecordingRepo::default();
        let wrapped = PowerCurveCompletedWorkoutRepository::new(inner.clone());

        let mut workout = sample_workout_with_watts(vec![200, 220]);
        workout.details_unavailable_reason = Some("no fit".to_string());
        assert!(workout.power_curve_5s.is_none());

        let result = wrapped.upsert(workout).await.unwrap();
        assert!(result.power_curve_5s.is_none());
    }

    #[tokio::test]
    async fn decorator_keeps_existing_curve_unchanged() {
        let inner = RecordingRepo::default();
        let wrapped = PowerCurveCompletedWorkoutRepository::new(inner.clone());

        let existing_curve = CompletedWorkoutPowerCurve {
            resolution_seconds: 5,
            sample_period_seconds: 1,
            source_samples: 10,
            valid_power_samples: 10,
            duration_start_seconds: 5,
            duration_step_seconds: 5,
            max_average_watts: vec![Some(999), Some(888)],
        };
        let mut workout =
            sample_workout_with_watts(vec![200, 220, 250, 240, 210, 190, 230, 260, 270, 255]);
        workout.power_curve_5s = Some(existing_curve.clone());

        let result = wrapped.upsert(workout).await.unwrap();
        assert_eq!(result.power_curve_5s, Some(existing_curve));
    }
}
