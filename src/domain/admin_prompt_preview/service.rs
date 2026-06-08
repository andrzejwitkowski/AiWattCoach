use std::sync::Arc;

use chrono::NaiveDate;

use crate::domain::{
    athlete_summary::AthleteSummaryRepository,
    coach_conversation::{
        assemble_calendar_coach_request, CalendarCoachPromptInput, CoachConversation,
        CoachConversationFocus, CoachConversationMessage, CoachConversationMessageRole,
        CoachConversationSurface,
    },
    completed_workouts::{CompletedWorkoutError, CompletedWorkoutReadUseCases},
    identity::Clock,
    llm::{current_date_string, preview_provider_messages, LlmChatRequest, UserLlmConfigProvider},
    llm_tools::{GetSelectedWorkoutDataPort, UpdatePlannedWorkoutDataPort},
    meso_cycle::{
        assemble_meso_cycle_coach_request, MesoCycleCoachPromptInput, MesoCycleLlmConfigPort,
        MesoCycleProjectionRepository, MesoCycleWindowPort,
    },
    planned_workouts::PlannedWorkoutRepository,
    settings::UserSettingsUseCases,
    special_days::SpecialDayRepository,
    training_context::{pick_representative_completed_workout_for_day, TrainingContextBuilder},
    workout_summary::{
        assemble_workout_summary_coach_request, try_load_meso_roadmap_stable_context,
        CompletedWorkoutTargetUseCases, WorkoutSummary, WorkoutSummaryCoachPromptInput,
        WorkoutSummaryRepository, ADMIN_PREVIEW_USER_MESSAGE,
    },
};

use super::{
    AdminPromptPreviewError, AdminPromptPreviewMeta, AdminPromptPreviewRequestBody,
    AdminPromptPreviewResponse, AdminPromptPreviewSurface, AdminPromptPreviewUseCases, BoxFuture,
};

#[derive(Clone)]
pub struct AdminPromptPreviewService<Time, Planned, Special>
where
    Time: Clock + Clone + Send + Sync + 'static,
    Planned: PlannedWorkoutRepository + Clone + Send + Sync + 'static,
    Special: SpecialDayRepository + Clone + Send + Sync + 'static,
{
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    completed_workout_read: Arc<dyn CompletedWorkoutReadUseCases>,
    planned_workout_repository: Option<Planned>,
    special_day_repository: Option<Special>,
    completed_workout_target_service: Arc<dyn CompletedWorkoutTargetUseCases>,
    workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
    athlete_summary_repository: Option<Arc<dyn AthleteSummaryRepository>>,
    settings_service: Arc<dyn UserSettingsUseCases>,
    data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
    planned_workout_update_port: Option<Arc<dyn UpdatePlannedWorkoutDataPort>>,
    meso_window_port: Option<Arc<dyn MesoCycleWindowPort>>,
    meso_llm_config_provider: Option<Arc<dyn MesoCycleLlmConfigPort>>,
    meso_projection_repository: Option<Arc<dyn MesoCycleProjectionRepository>>,
    clock: Time,
}

impl<Time, Planned, Special> AdminPromptPreviewService<Time, Planned, Special>
where
    Time: Clock + Clone + Send + Sync + 'static,
    Planned: PlannedWorkoutRepository + Clone + Send + Sync + 'static,
    Special: SpecialDayRepository + Clone + Send + Sync + 'static,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "admin prompt preview composes existing production ports without a new facade"
    )]
    pub fn new(
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
        completed_workout_read: Arc<dyn CompletedWorkoutReadUseCases>,
        planned_workout_repository: Option<Planned>,
        special_day_repository: Option<Special>,
        completed_workout_target_service: Arc<dyn CompletedWorkoutTargetUseCases>,
        workout_summary_repository: Arc<dyn WorkoutSummaryRepository>,
        athlete_summary_repository: Option<Arc<dyn AthleteSummaryRepository>>,
        settings_service: Arc<dyn UserSettingsUseCases>,
        data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
        planned_workout_update_port: Option<Arc<dyn UpdatePlannedWorkoutDataPort>>,
        meso_window_port: Option<Arc<dyn MesoCycleWindowPort>>,
        meso_llm_config_provider: Option<Arc<dyn MesoCycleLlmConfigPort>>,
        meso_projection_repository: Option<Arc<dyn MesoCycleProjectionRepository>>,
        clock: Time,
    ) -> Self {
        Self {
            training_context_builder,
            llm_config_provider,
            completed_workout_read,
            planned_workout_repository,
            special_day_repository,
            completed_workout_target_service,
            workout_summary_repository,
            athlete_summary_repository,
            settings_service,
            data_port,
            planned_workout_update_port,
            meso_window_port,
            meso_llm_config_provider,
            meso_projection_repository,
            clock,
        }
    }

    fn validate_date(&self, date: &str) -> Result<NaiveDate, AdminPromptPreviewError> {
        let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| AdminPromptPreviewError::InvalidDate)?;
        let today = NaiveDate::parse_from_str(&current_date_string(&self.clock), "%Y-%m-%d")
            .map_err(|_| AdminPromptPreviewError::InvalidDate)?;
        if parsed > today {
            return Err(AdminPromptPreviewError::FutureDate);
        }
        Ok(parsed)
    }

    fn map_response(input: MappedPreviewResponse<'_>) -> AdminPromptPreviewResponse {
        let provider_messages = preview_provider_messages(&input.request);
        AdminPromptPreviewResponse {
            meta: AdminPromptPreviewMeta {
                user_id: input.user_id.to_string(),
                date: input.date.to_string(),
                surface: input.surface.as_str().to_string(),
                provider: input.provider.to_string(),
                model: input.model.to_string(),
                focus_date: input.date.to_string(),
                selected_workout_id: input
                    .post_workout
                    .as_ref()
                    .map(|meta| meta.selected_workout_id.clone()),
                selection_method: input
                    .post_workout
                    .as_ref()
                    .map(|meta| meta.selection_method.clone()),
                compliance_score: input.post_workout.and_then(|meta| meta.compliance_score),
                meso_start: input.meso.as_ref().map(|meta| meta.meso_start.clone()),
                meso_end: input.meso.as_ref().map(|meta| meta.meso_end.clone()),
                ai_coach_last_date: input
                    .meso
                    .as_ref()
                    .and_then(|meta| meta.ai_coach_last_date.clone()),
            },
            request: AdminPromptPreviewRequestBody {
                system_prompt: input.request.system_prompt,
                stable_context: input.request.stable_context,
                volatile_context: input.request.volatile_context,
                conversation: input.request.conversation,
                tools: input.request.tools,
                tool_choice: input.request.tool_choice,
            },
            provider_messages,
        }
    }

    async fn load_planned_workouts(
        &self,
        user_id: &str,
        date: &str,
    ) -> Result<Vec<crate::domain::planned_workouts::PlannedWorkout>, AdminPromptPreviewError> {
        let Some(repository) = self.planned_workout_repository.as_ref() else {
            return Ok(Vec::new());
        };
        repository
            .list_by_user_id_and_date_range(user_id, date, date)
            .await
            .map_err(map_planned_error)
    }

    async fn load_special_days(
        &self,
        user_id: &str,
        date: &str,
    ) -> Result<Vec<crate::domain::special_days::SpecialDay>, AdminPromptPreviewError> {
        let Some(repository) = self.special_day_repository.as_ref() else {
            return Ok(Vec::new());
        };
        repository
            .list_by_user_id_and_date_range(user_id, date, date)
            .await
            .map_err(map_special_day_error)
    }

    async fn load_athlete_summary_text(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, AdminPromptPreviewError> {
        let Some(repository) = self.athlete_summary_repository.as_ref() else {
            return Ok(None);
        };
        let summary = repository
            .find_by_user_id(user_id)
            .await
            .map_err(|error| AdminPromptPreviewError::Repository(error.to_string()))?;
        Ok(summary.map(|value| value.summary_text))
    }
}

fn map_planned_error(
    error: crate::domain::planned_workouts::PlannedWorkoutError,
) -> AdminPromptPreviewError {
    AdminPromptPreviewError::Repository(error.to_string())
}

fn map_special_day_error(
    error: crate::domain::special_days::SpecialDayError,
) -> AdminPromptPreviewError {
    AdminPromptPreviewError::Repository(error.to_string())
}

fn map_completed_workout_error(error: CompletedWorkoutError) -> AdminPromptPreviewError {
    AdminPromptPreviewError::Repository(error.to_string())
}

impl<Time, Planned, Special> AdminPromptPreviewUseCases
    for AdminPromptPreviewService<Time, Planned, Special>
where
    Time: Clock + Clone + Send + Sync + 'static,
    Planned: PlannedWorkoutRepository + Clone + Send + Sync + 'static,
    Special: SpecialDayRepository + Clone + Send + Sync + 'static,
{
    fn preview_post_workout(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<AdminPromptPreviewResponse, AdminPromptPreviewError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move { service.preview_post_workout_impl(&user_id, &date).await })
    }

    fn preview_calendar_coach(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<AdminPromptPreviewResponse, AdminPromptPreviewError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move { service.preview_calendar_coach_impl(&user_id, &date).await })
    }

    fn preview_meso_cycle_coach(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<AdminPromptPreviewResponse, AdminPromptPreviewError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move { service.preview_meso_cycle_coach_impl(&user_id, &date).await })
    }
}

impl<Time, Planned, Special> AdminPromptPreviewService<Time, Planned, Special>
where
    Time: Clock + Clone + Send + Sync + 'static,
    Planned: PlannedWorkoutRepository + Clone + Send + Sync + 'static,
    Special: SpecialDayRepository + Clone + Send + Sync + 'static,
{
    async fn preview_post_workout_impl(
        &self,
        user_id: &str,
        date: &str,
    ) -> Result<AdminPromptPreviewResponse, AdminPromptPreviewError> {
        let focus_date = self.validate_date(date)?;
        let preview_epoch = preview_focus_date_epoch_seconds(focus_date);

        let settings = self
            .settings_service
            .get_settings(user_id)
            .await
            .map_err(|error| AdminPromptPreviewError::Settings(error.to_string()))?;

        let day_workouts = self
            .completed_workout_read
            .list_completed_workouts(user_id, date, date)
            .await
            .map_err(map_completed_workout_error)?;
        let planned_workouts = self.load_planned_workouts(user_id, date).await?;
        let special_days = self.load_special_days(user_id, date).await?;
        let pick = pick_representative_completed_workout_for_day(
            day_workouts,
            &std::collections::HashSet::new(),
            &planned_workouts,
            &special_days,
            settings
                .cycling
                .ftp_watts
                .map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
        )
        .ok_or(AdminPromptPreviewError::NoCompletedWorkoutForDate)?;

        let resolved = self
            .completed_workout_target_service
            .resolve_completed_workout_target(user_id, &pick.workout.completed_workout_id)
            .await
            .map_err(|error| AdminPromptPreviewError::TargetResolution(error.to_string()))?
            .ok_or(AdminPromptPreviewError::NoCompletedWorkoutForDate)?;
        let workout_id = resolved.preferred_workout_id;

        let training_context = self
            .training_context_builder
            .build_as_of(user_id, &workout_id, focus_date)
            .await
            .map_err(AdminPromptPreviewError::Llm)?;

        let summary = self
            .workout_summary_repository
            .find_by_user_id_and_workout_id(user_id, &workout_id)
            .await
            .map_err(|error| AdminPromptPreviewError::Repository(error.to_string()))?
            .unwrap_or_else(|| preview_workout_summary(user_id, &workout_id, preview_epoch));

        let athlete_summary_text = self.load_athlete_summary_text(user_id).await?;
        let config = self
            .llm_config_provider
            .get_config(user_id)
            .await
            .map_err(AdminPromptPreviewError::Llm)?;

        let meso_roadmap_stable_context =
            if let Some(repository) = self.meso_projection_repository.as_deref() {
                try_load_meso_roadmap_stable_context(repository, user_id).await
            } else {
                None
            };
        let request = assemble_workout_summary_coach_request(WorkoutSummaryCoachPromptInput {
            user_id: user_id.to_string(),
            config: config.clone(),
            summary,
            training_context,
            user_message: ADMIN_PREVIEW_USER_MESSAGE.to_string(),
            athlete_summary_text,
            conversation_epoch_seconds: preview_epoch,
            today: date.to_string(),
            data_port: self.data_port.clone(),
            reusable_cache_id: None,
            meso_roadmap_stable_context,
        });

        Ok(Self::map_response(MappedPreviewResponse {
            surface: AdminPromptPreviewSurface::PostWorkout,
            user_id,
            date,
            provider: config.provider.as_str(),
            model: &config.model,
            request,
            post_workout: Some(PostWorkoutPreviewMeta {
                selected_workout_id: workout_id,
                selection_method: pick.method.as_str().to_string(),
                compliance_score: pick.compliance_score,
            }),
            meso: None,
        }))
    }

    async fn preview_calendar_coach_impl(
        &self,
        user_id: &str,
        date: &str,
    ) -> Result<AdminPromptPreviewResponse, AdminPromptPreviewError> {
        let focus_date = self.validate_date(date)?;
        let preview_epoch = preview_focus_date_epoch_seconds(focus_date);

        let training_context = self
            .training_context_builder
            .build_calendar_overview_context_as_of(user_id, focus_date)
            .await
            .map_err(AdminPromptPreviewError::Llm)?;
        let config = self
            .llm_config_provider
            .get_config(user_id)
            .await
            .map_err(AdminPromptPreviewError::Llm)?;

        let conversation = CoachConversation::new(
            "admin-preview".to_string(),
            user_id.to_string(),
            CoachConversationSurface::Calendar,
            CoachConversationFocus::Overview,
            preview_epoch,
        );
        let preview_message = CoachConversationMessage {
            id: "admin-preview-user".to_string(),
            conversation_id: conversation.conversation_id.clone(),
            user_id: user_id.to_string(),
            role: CoachConversationMessageRole::User,
            content: ADMIN_PREVIEW_USER_MESSAGE.to_string(),
            tool_call: None,
            reasoning_content: None,
            created_at_epoch_seconds: preview_epoch,
        };

        let request = assemble_calendar_coach_request(CalendarCoachPromptInput {
            user_id: user_id.to_string(),
            config: config.clone(),
            conversation,
            messages: vec![preview_message],
            training_context,
            preview_message_id: "admin-preview-user".to_string(),
            conversation_epoch_seconds: preview_epoch,
            latest_user_message_epoch_seconds: Some(preview_epoch),
            today: date.to_string(),
            data_port: self.data_port.clone(),
            planned_workout_update_port: self.planned_workout_update_port.clone(),
        });

        Ok(Self::map_response(MappedPreviewResponse {
            surface: AdminPromptPreviewSurface::CalendarCoach,
            user_id,
            date,
            provider: config.provider.as_str(),
            model: &config.model,
            request,
            post_workout: None,
            meso: None,
        }))
    }

    async fn preview_meso_cycle_coach_impl(
        &self,
        user_id: &str,
        date: &str,
    ) -> Result<AdminPromptPreviewResponse, AdminPromptPreviewError> {
        let focus_date = self.validate_date(date)?;
        let preview_epoch = preview_focus_date_epoch_seconds(focus_date);
        let meso_window_port =
            self.meso_window_port
                .as_ref()
                .ok_or(AdminPromptPreviewError::MesoCycle(
                    crate::domain::meso_cycle::MesoCycleError::Unavailable(
                        "meso cycle preview is not configured".to_string(),
                    ),
                ))?;
        let meso_llm_config_provider =
            self.meso_llm_config_provider
                .as_ref()
                .ok_or(AdminPromptPreviewError::MesoCycle(
                    crate::domain::meso_cycle::MesoCycleError::Unavailable(
                        "meso cycle preview is not configured".to_string(),
                    ),
                ))?;

        let window = meso_window_port
            .resolve_window(user_id, date)
            .await
            .map_err(AdminPromptPreviewError::MesoCycle)?;
        let meso_end = NaiveDate::parse_from_str(&window.meso_end, "%Y-%m-%d")
            .map_err(|_| AdminPromptPreviewError::InvalidDate)?;
        let training_context = self
            .training_context_builder
            .build_meso_cycle_context(user_id, meso_end)
            .await
            .map_err(AdminPromptPreviewError::Llm)?;
        let config = meso_llm_config_provider
            .get_meso_cycle_config(user_id)
            .await
            .map_err(AdminPromptPreviewError::MesoCycle)?;
        let bundle = assemble_meso_cycle_coach_request(MesoCycleCoachPromptInput {
            user_id: user_id.to_string(),
            config: config.clone(),
            window: window.clone(),
            training_context,
            conversation_epoch_seconds: preview_epoch,
            today: date.to_string(),
            data_port: self.data_port.clone(),
        });

        Ok(Self::map_response(MappedPreviewResponse {
            surface: AdminPromptPreviewSurface::MesoCycleCoach,
            user_id,
            date,
            provider: config.provider.as_str(),
            model: &config.model,
            request: bundle.request,
            post_workout: None,
            meso: Some(MesoPreviewMeta {
                meso_start: window.meso_start,
                meso_end: window.meso_end,
                ai_coach_last_date: window.ai_coach_last_date,
            }),
        }))
    }
}

struct MappedPreviewResponse<'a> {
    surface: AdminPromptPreviewSurface,
    user_id: &'a str,
    date: &'a str,
    provider: &'a str,
    model: &'a str,
    request: LlmChatRequest,
    post_workout: Option<PostWorkoutPreviewMeta>,
    meso: Option<MesoPreviewMeta>,
}

struct PostWorkoutPreviewMeta {
    selected_workout_id: String,
    selection_method: String,
    compliance_score: Option<f64>,
}

struct MesoPreviewMeta {
    meso_start: String,
    meso_end: String,
    ai_coach_last_date: Option<String>,
}

fn preview_focus_date_epoch_seconds(date: NaiveDate) -> i64 {
    date.and_hms_opt(12, 0, 0)
        .expect("valid preview focus timestamp")
        .and_utc()
        .timestamp()
}

fn preview_workout_summary(user_id: &str, workout_id: &str, now: i64) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("preview-{workout_id}"),
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        rpe: None,
        messages: Vec::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: now,
        updated_at_epoch_seconds: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::epoch_seconds_to_rfc3339;

    #[test]
    fn preview_focus_date_uses_noon_utc_to_avoid_next_day_in_positive_timezones() {
        let focus_date = NaiveDate::from_ymd_opt(2026, 6, 7).expect("valid date");
        let epoch = preview_focus_date_epoch_seconds(focus_date);

        assert_eq!(epoch_seconds_to_rfc3339(epoch), "2026-06-07T12:00:00+00:00");
    }
}
