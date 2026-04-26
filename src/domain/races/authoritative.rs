use std::collections::HashSet;

use crate::domain::external_sync::{
    CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservationRepository,
};

use super::{BoxFuture, Race, RaceError, RaceRepository};

#[derive(Clone)]
pub struct AuthoritativeRaceRepository<Races, Observations>
where
    Races: RaceRepository + Clone,
    Observations: ExternalObservationRepository + Clone,
{
    races: Races,
    observations: Observations,
}

impl<Races, Observations> AuthoritativeRaceRepository<Races, Observations>
where
    Races: RaceRepository + Clone,
    Observations: ExternalObservationRepository + Clone,
{
    pub fn new(races: Races, observations: Observations) -> Self {
        Self {
            races,
            observations,
        }
    }

    async fn filter_visible(
        observations: Observations,
        user_id: &str,
        races: Vec<Race>,
    ) -> Result<Vec<Race>, RaceError> {
        if races.is_empty() {
            return Ok(Vec::new());
        }

        let canonical_entities = races
            .iter()
            .map(|race| CanonicalEntityRef::new(CanonicalEntityKind::Race, race.race_id.clone()))
            .collect::<Vec<_>>();
        let hidden_ids = observations
            .find_by_canonical_entities(user_id, ExternalObjectKind::Race, &canonical_entities)
            .await
            .map_err(|error| RaceError::Internal(error.to_string()))?
            .into_iter()
            .map(|observation| observation.canonical_entity.entity_id)
            .collect::<HashSet<_>>();

        Ok(races
            .into_iter()
            .filter(|race| !hidden_ids.contains(&race.race_id))
            .collect())
    }
}

impl<Races, Observations> RaceRepository for AuthoritativeRaceRepository<Races, Observations>
where
    Races: RaceRepository + Clone,
    Observations: ExternalObservationRepository + Clone,
{
    fn list_by_user_id(&self, user_id: &str) -> BoxFuture<Result<Vec<Race>, RaceError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let races = repository.races.list_by_user_id(&user_id).await?;
            Self::filter_visible(repository.observations.clone(), &user_id, races).await
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &crate::domain::intervals::DateRange,
    ) -> BoxFuture<Result<Vec<Race>, RaceError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let races = repository
                .races
                .list_by_user_id_and_range(&user_id, &range)
                .await?;
            Self::filter_visible(repository.observations.clone(), &user_id, races).await
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> BoxFuture<Result<Option<Race>, RaceError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            let Some(race) = repository
                .races
                .find_by_user_id_and_race_id(&user_id, &race_id)
                .await?
            else {
                return Ok(None);
            };
            Self::filter_visible(repository.observations.clone(), &user_id, vec![race])
                .await
                .map(|mut races| races.pop())
        })
    }

    fn upsert(&self, race: Race) -> BoxFuture<Result<Race, RaceError>> {
        self.races.upsert(race)
    }

    fn delete(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<(), RaceError>> {
        self.races.delete(user_id, race_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::external_sync::{
        ExternalObservation, ExternalObservationParams, ExternalObservationRepository,
        ExternalProvider, ExternalSyncRepositoryError,
    };

    use super::*;

    #[derive(Clone, Default)]
    struct TestObservations {
        observations: Vec<ExternalObservation>,
    }

    #[derive(Clone, Default)]
    struct TestRaces {
        races: Vec<Race>,
    }

    impl RaceRepository for TestRaces {
        fn list_by_user_id(&self, user_id: &str) -> BoxFuture<Result<Vec<Race>, RaceError>> {
            let races = self.races.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                Ok(races
                    .into_iter()
                    .filter(|race| race.user_id == user_id)
                    .collect())
            })
        }

        fn list_by_user_id_and_range(
            &self,
            user_id: &str,
            range: &crate::domain::intervals::DateRange,
        ) -> BoxFuture<Result<Vec<Race>, RaceError>> {
            let races = self.races.clone();
            let user_id = user_id.to_string();
            let oldest = range.oldest.clone();
            let newest = range.newest.clone();
            Box::pin(async move {
                Ok(races
                    .into_iter()
                    .filter(|race| race.user_id == user_id)
                    .filter(|race| race.date >= oldest && race.date <= newest)
                    .collect())
            })
        }

        fn find_by_user_id_and_race_id(
            &self,
            user_id: &str,
            race_id: &str,
        ) -> BoxFuture<Result<Option<Race>, RaceError>> {
            let races = self.races.clone();
            let user_id = user_id.to_string();
            let race_id = race_id.to_string();
            Box::pin(async move {
                Ok(races
                    .into_iter()
                    .find(|race| race.user_id == user_id && race.race_id == race_id))
            })
        }

        fn upsert(&self, race: Race) -> BoxFuture<Result<Race, RaceError>> {
            Box::pin(async move { Ok(race) })
        }

        fn delete(&self, _user_id: &str, _race_id: &str) -> BoxFuture<Result<(), RaceError>> {
            Box::pin(async { Ok(()) })
        }
    }

    impl ExternalObservationRepository for TestObservations {
        fn upsert(
            &self,
            observation: ExternalObservation,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<ExternalObservation, ExternalSyncRepositoryError>,
        > {
            Box::pin(async move { Ok(observation) })
        }

        fn find_by_canonical_entities(
            &self,
            user_id: &str,
            external_object_kind: ExternalObjectKind,
            canonical_entities: &[CanonicalEntityRef],
        ) -> crate::domain::external_sync::BoxFuture<
            Result<Vec<ExternalObservation>, ExternalSyncRepositoryError>,
        > {
            let observations = self.observations.clone();
            let user_id = user_id.to_string();
            let canonical_entities = canonical_entities.to_vec();
            Box::pin(async move {
                Ok(observations
                    .into_iter()
                    .filter(|observation| {
                        observation.user_id == user_id
                            && observation.external_object_kind == external_object_kind
                            && canonical_entities.contains(&observation.canonical_entity)
                    })
                    .collect())
            })
        }

        fn find_by_provider_and_external_id(
            &self,
            _user_id: &str,
            _provider: ExternalProvider,
            _external_id: &str,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<Option<ExternalObservation>, ExternalSyncRepositoryError>,
        > {
            Box::pin(async { Ok(None) })
        }

        fn find_by_dedup_key(
            &self,
            _user_id: &str,
            _external_object_kind: ExternalObjectKind,
            _dedup_key: &str,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<Vec<ExternalObservation>, ExternalSyncRepositoryError>,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    fn sample_race(race_id: &str, date: &str) -> Race {
        Race {
            race_id: race_id.to_string(),
            user_id: "user-1".to_string(),
            date: date.to_string(),
            name: format!("Race {race_id}"),
            distance_meters: 10_000,
            discipline: super::super::RaceDiscipline::Road,
            priority: super::super::RacePriority::A,
            result: None,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        }
    }

    #[tokio::test]
    async fn hides_externally_imported_races_from_reads() {
        let repository = AuthoritativeRaceRepository::new(
            TestRaces {
                races: vec![
                    sample_race("race-local", "2026-05-01"),
                    sample_race("race-imported", "2026-05-02"),
                ],
            },
            TestObservations {
                observations: vec![ExternalObservation::new(ExternalObservationParams {
                    user_id: "user-1".to_string(),
                    provider: ExternalProvider::Intervals,
                    external_object_kind: ExternalObjectKind::Race,
                    external_id: "event-1".to_string(),
                    canonical_entity: CanonicalEntityRef::new(
                        CanonicalEntityKind::Race,
                        "race-imported".to_string(),
                    ),
                    normalized_payload_hash: None,
                    dedup_key: None,
                    observed_at_epoch_seconds: 1,
                })],
            },
        );

        let races = repository.list_by_user_id("user-1").await.unwrap();

        assert_eq!(races.len(), 1);
        assert_eq!(races[0].race_id, "race-local");
        assert!(repository
            .find_by_user_id_and_race_id("user-1", "race-imported")
            .await
            .unwrap()
            .is_none());
    }
}
