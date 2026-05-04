use crate::domain::{
    intervals::DateRange,
    races::{
        BoxFuture as RaceBoxFuture, Race, RaceDiscipline, RaceError, RacePriority, RaceRepository,
    },
    special_days::{SpecialDay, SpecialDayKind, SpecialDayRepository},
};

#[derive(Clone)]
pub(crate) struct TestSpecialDayRepository {
    days: Vec<SpecialDay>,
}

impl Default for TestSpecialDayRepository {
    fn default() -> Self {
        Self {
            days: vec![SpecialDay::new(
                "intervals-special-day:202".to_string(),
                "user-1".to_string(),
                "2026-04-02".to_string(),
                SpecialDayKind::Note,
                Some("Sick day".to_string()),
                Some("Felt unwell with sore throat".to_string()),
            )
            .unwrap()],
        }
    }
}

impl SpecialDayRepository for TestSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::special_days::BoxFuture<
        Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>,
    > {
        let days = self.days.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::special_days::BoxFuture<
        Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>,
    > {
        let days = self.days.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id)
                .filter(|day| day.date >= oldest && day.date <= newest)
                .collect())
        })
    }

    fn upsert(
        &self,
        _special_day: SpecialDay,
    ) -> crate::domain::special_days::BoxFuture<
        Result<SpecialDay, crate::domain::special_days::SpecialDayError>,
    > {
        unreachable!()
    }
}

#[derive(Clone)]
pub(crate) struct TestRaceRepository;

impl RaceRepository for TestRaceRepository {
    fn list_by_user_id(&self, user_id: &str) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(vec![Race {
                race_id: "race-1".to_string(),
                user_id,
                date: "2026-05-10".to_string(),
                name: "Spring Classic".to_string(),
                distance_meters: 123_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
                result: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }])
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(vec![Race {
                race_id: "race-1".to_string(),
                user_id,
                date: "2026-05-10".to_string(),
                name: "Spring Classic".to_string(),
                distance_meters: 123_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
                result: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }]
            .into_iter()
            .filter(|race| race.date >= oldest && race.date <= newest)
            .collect())
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        _user_id: &str,
        _race_id: &str,
    ) -> RaceBoxFuture<Result<Option<Race>, RaceError>> {
        unreachable!()
    }

    fn upsert(&self, _race: Race) -> RaceBoxFuture<Result<Race, RaceError>> {
        unreachable!()
    }

    fn delete(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        unreachable!()
    }
}
