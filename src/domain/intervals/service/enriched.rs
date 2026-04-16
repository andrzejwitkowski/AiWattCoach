use super::*;

impl<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
    PocRepo: PestParserPocRepositoryPort,
    Time: Clock,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort,
{
    pub(super) async fn get_enriched_event_impl(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> Result<EnrichedEvent, IntervalsError> {
        let configured_ftp_watts = self.settings.get_cycling_ftp_watts(user_id).await?;
        let credentials = self.settings.get_credentials(user_id).await?;
        let event = self.api.get_event(&credentials, event_id).await?;
        let date_key = event
            .start_date_local
            .split('T')
            .next()
            .unwrap_or(&event.start_date_local)
            .to_string();
        let listed_activities = self
            .api
            .list_activities(
                &credentials,
                &DateRange {
                    oldest: date_key.clone(),
                    newest: date_key,
                },
            )
            .await?;
        let effective_ftp_watts = configured_ftp_watts.or_else(|| {
            listed_activities
                .iter()
                .find_map(|activity| activity.metrics.ftp_watts)
        });
        let parsed_workout =
            parse_workout_doc(event.structured_workout_text(), effective_ftp_watts);

        let mut best_match =
            find_best_activity_match(&parsed_workout, &listed_activities, effective_ftp_watts);

        for listed_activity in &listed_activities {
            let detailed_activity = match self
                .api
                .get_activity(&credentials, &listed_activity.id)
                .await
            {
                Ok(activity) => activity,
                Err(_) => continue,
            };

            let candidate = match find_best_activity_match(
                &parsed_workout,
                std::slice::from_ref(&detailed_activity),
                effective_ftp_watts,
            ) {
                Some(candidate) => candidate,
                None => continue,
            };

            if best_match.as_ref().is_none_or(|current| {
                candidate.compliance_score > current.compliance_score
                    || (candidate.compliance_score == current.compliance_score
                        && candidate.power_values.len() > current.power_values.len())
            }) {
                best_match = Some(candidate);
            }
        }

        Ok(EnrichedEvent {
            event,
            parsed_workout,
            actual_workout: best_match,
        })
    }
}
