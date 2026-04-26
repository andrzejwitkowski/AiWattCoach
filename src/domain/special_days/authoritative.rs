use std::collections::HashSet;

use crate::domain::external_sync::{
    CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservationRepository,
};

use super::{BoxFuture, SpecialDay, SpecialDayError, SpecialDayRepository};

#[derive(Clone)]
pub struct AuthoritativeSpecialDayRepository<SpecialDays, Observations>
where
    SpecialDays: SpecialDayRepository + Clone,
    Observations: ExternalObservationRepository + Clone,
{
    special_days: SpecialDays,
    observations: Observations,
}

impl<SpecialDays, Observations> AuthoritativeSpecialDayRepository<SpecialDays, Observations>
where
    SpecialDays: SpecialDayRepository + Clone,
    Observations: ExternalObservationRepository + Clone,
{
    pub fn new(special_days: SpecialDays, observations: Observations) -> Self {
        Self {
            special_days,
            observations,
        }
    }

    async fn filter_visible(
        observations: Observations,
        user_id: &str,
        special_days: Vec<SpecialDay>,
    ) -> Result<Vec<SpecialDay>, SpecialDayError> {
        if special_days.is_empty() {
            return Ok(Vec::new());
        }

        let canonical_entities = special_days
            .iter()
            .map(|day| {
                CanonicalEntityRef::new(CanonicalEntityKind::SpecialDay, day.special_day_id.clone())
            })
            .collect::<Vec<_>>();
        let hidden_ids = observations
            .find_by_canonical_entities(
                user_id,
                ExternalObjectKind::SpecialDay,
                &canonical_entities,
            )
            .await
            .map_err(|error| SpecialDayError::Repository(error.to_string()))?
            .into_iter()
            .map(|observation| observation.canonical_entity.entity_id)
            .collect::<HashSet<_>>();

        Ok(special_days
            .into_iter()
            .filter(|day| !hidden_ids.contains(&day.special_day_id))
            .collect())
    }
}

impl<SpecialDays, Observations> SpecialDayRepository
    for AuthoritativeSpecialDayRepository<SpecialDays, Observations>
where
    SpecialDays: SpecialDayRepository + Clone,
    Observations: ExternalObservationRepository + Clone,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let special_days = repository.special_days.list_by_user_id(&user_id).await?;
            Self::filter_visible(repository.observations.clone(), &user_id, special_days).await
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let special_days = repository
                .special_days
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await?;
            Self::filter_visible(repository.observations.clone(), &user_id, special_days).await
        })
    }

    fn upsert(&self, special_day: SpecialDay) -> BoxFuture<Result<SpecialDay, SpecialDayError>> {
        self.special_days.upsert(special_day)
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

    fn sample_day(id: &str, date: &str) -> SpecialDay {
        SpecialDay::new(
            id.to_string(),
            "user-1".to_string(),
            date.to_string(),
            super::super::SpecialDayKind::Illness,
            Some("Illness".to_string()),
            Some("Recovery".to_string()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn hides_externally_imported_special_days_from_reads() {
        let repository = AuthoritativeSpecialDayRepository::new(
            super::super::ports::NoopSpecialDayRepository::default(),
            TestObservations {
                observations: vec![ExternalObservation::new(ExternalObservationParams {
                    user_id: "user-1".to_string(),
                    provider: ExternalProvider::Intervals,
                    external_object_kind: ExternalObjectKind::SpecialDay,
                    external_id: "event-2".to_string(),
                    canonical_entity: CanonicalEntityRef::new(
                        CanonicalEntityKind::SpecialDay,
                        "special-imported".to_string(),
                    ),
                    normalized_payload_hash: None,
                    dedup_key: None,
                    observed_at_epoch_seconds: 1,
                })],
            },
        );
        repository
            .upsert(sample_day("special-local", "2026-05-01"))
            .await
            .unwrap();
        repository
            .upsert(sample_day("special-imported", "2026-05-02"))
            .await
            .unwrap();

        let days = repository
            .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
            .await
            .unwrap();

        assert_eq!(days.len(), 1);
        assert_eq!(days[0].special_day_id, "special-local");
    }
}
