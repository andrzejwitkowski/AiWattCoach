use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutRepository},
    planned_workouts::{PlannedWorkout as CanonicalPlannedWorkout, PlannedWorkoutRepository},
};

use super::workout_fixtures::{sample_completed_workout_on_date_with_ftp, sample_planned_workout};

#[derive(Clone)]
pub(crate) struct TestCompletedWorkoutRepository {
    workouts: Vec<CompletedWorkout>,
}

impl Default for TestCompletedWorkoutRepository {
    fn default() -> Self {
        Self {
            workouts: vec![sample_completed_workout_on_date_with_ftp(
                "ride-1",
                "2026-04-03T08:00:00",
                Some(300),
                Some("intervals-event:101".to_string()),
            )],
        }
    }
}

impl TestCompletedWorkoutRepository {
    pub(crate) fn with_workouts(workouts: Vec<CompletedWorkout>) -> Self {
        Self { workouts }
    }
}

impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(workouts.into_iter().find(|workout| {
                workout.user_id == user_id && workout.completed_workout_id == completed_workout_id
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            Ok(workouts.into_iter().find(|workout| {
                workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let mut workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            workouts.retain(|workout| workout.user_id == user_id);
            workouts.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(workouts.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .collect())
        })
    }

    fn upsert(
        &self,
        _workout: CompletedWorkout,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(crate) struct TestPlannedWorkoutRepository {
    workouts: Vec<CanonicalPlannedWorkout>,
}

impl Default for TestPlannedWorkoutRepository {
    fn default() -> Self {
        Self {
            workouts: vec![
                sample_planned_workout(101, "2026-04-03"),
                sample_planned_workout(303, "2026-04-25"),
            ],
        }
    }
}

impl PlannedWorkoutRepository for TestPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<Vec<CanonicalPlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<Vec<CanonicalPlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let workouts = self.workouts.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .collect())
        })
    }

    fn upsert(
        &self,
        _workout: CanonicalPlannedWorkout,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<CanonicalPlannedWorkout, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        unreachable!()
    }
}
