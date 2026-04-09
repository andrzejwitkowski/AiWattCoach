use std::sync::Arc;

use serde_json::json;

use super::context_prelude::PACKED_TRAINING_CONTEXT_LEGEND;

use crate::domain::{
    ai_workflow::ValidationIssue,
    identity::Clock,
    llm::{
        BoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmError, LlmMessageRole,
        UserLlmConfigProvider,
    },
    training_context::TrainingContextBuilder,
    training_plan::{TrainingPlanError, TrainingPlanGenerator},
    workout_summary::WorkoutRecap,
};

const TRAINING_PLAN_RECAP_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach generating a completed workout recap from packed training context. Use only the provided context, stay factual, concise, and avoid inventing details.";
const TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach and a strict syntax generator for Intervals.icu planned workouts. Generate a 14-day internal cycling plan window using only the backend-supported workout grammar. Use the packed training context and the completed workout recap as the planning basis.";
const TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach and a strict syntax generator for Intervals.icu planned workouts. Help correct invalid dated workout sections using only the backend-supported workout grammar. Only rewrite the invalid dated sections provided.";
const TRAINING_PLAN_OUTPUT_GRAMMAR: &str = "Critical rules: Output ONLY the raw workout text. Do not include commentary, greetings, markdown fences, or prose before/after the dated sections. Every actionable workout step MUST begin with a hyphen followed by a space (`- `). Do not invent syntax. Output grammar: One dated section per day. Start each section with a YYYY-MM-DD line. Follow with either `Rest Day` or workout-builder text lines. Block titles and descriptions are allowed on lines that do not start with `- ` and do not end with `x`. Step syntax: `- [Duration] [Target]`. Ramp syntax: `- [Duration] ramp [Start Target]-[End Target]`. Repeat headers must end with `x`, such as `Main Set 4x`. Supported durations: `30s`, `5m`, `45m`. Supported targets: `65%`, `95-105%`, `120-160W`. Example step: `- 45m 65%`. Example output: `2026-04-06\nWarmup\n- 15m ramp 100-270W\n2026-04-07\nRest Day`. Do not use cadence, zone targets, inline text cues, hour units, or distance units because the current backend parser does not accept them. For correction prompts, Only output corrected dated sections for the invalid dates you are fixing.";
const TRAINING_PLAN_PLANNING_GUIDELINES_BASE: &str = "Planning guidelines: Follow a durability-first approach. Road cycling, especially masters racing, is stochastic; prioritize power repeatability and lactate clearance over pure steady-state aerobic work. Treat athlete age 45+, body-weight changes, and medications such as beta-blockers as fixed environmental constraints, not pathologies. Metric hierarchy: RPE over power over TSS/TSB over heart rate. If RPE stays low or moderate despite high fatigue metrics, trust recovery capacity and maintain load. Ignore heart rate for intensity pacing when beta-blockers are present. Never prescribe more than 2 consecutive Rest Day entries unless the athlete explicitly reports illness or injury. During build phases, TSB/Form may sit in the -15 to -25 range without forcing emergency rest. Prevent detraining by preferring Active Recovery over total inactivity when extra recovery is needed. Treat races as Category C by default unless the context explicitly says otherwise. For Category C races, do not taper: treat the race like a high-intensity stochastic interval session, keep normal training load during race week, keep Tuesday and Wednesday interval sessions before a Sunday race when the context supports it, allow at most one light spinning or Rest Day on Friday or Saturday before the race, and schedule recovery or light endurance the day after the race before returning to structured intervals within 48 hours. When race time is materially earlier than normal training time, gradually shift key sessions toward the race start window to support circadian rhythm and heat adaptation.";
const TRAINING_PLAN_AVAILABILITY_CONFIGURED_GUIDANCE: &str = "Weekly availability is mandatory and must be respected: only schedule workouts on weekdays marked available, keep unavailable days as Rest Day, and never exceed the configured max duration minutes for each available weekday.";
const TRAINING_PLAN_AVAILABILITY_UNCONFIGURED_GUIDANCE: &str = "Weekly availability is not configured in this context. Do not infer unavailable days or extra rest constraints from missing availability data. Plan a sensible 14-day cycling window from the training context alone, and avoid claiming that weekly availability is configured.";

#[derive(Clone)]
pub struct TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    clock: Time,
}

impl<Time> TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    pub fn new(
        llm_chat_port: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        clock: Time,
    ) -> Self {
        Self {
            llm_chat_port,
            llm_config_provider,
            training_context_builder,
            clock,
        }
    }
}

impl<Time> TrainingPlanGenerator for TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    fn generate_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let stable_context = format!(
                "saved_at_epoch_seconds={saved_at_epoch_seconds}\ntraining_plan_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let user_prompt = "Generate a concise workout recap for the completed workout. Focus on execution quality, response to the session, and what matters for planning the next training window.";

            let response = llm_chat_port
                .chat(
                    config.clone(),
                    LlmChatRequest {
                        user_id,
                        system_prompt: training_plan_recap_system_prompt(),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage {
                            role: LlmMessageRole::User,
                            content: user_prompt.to_string(),
                        }],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    },
                )
                .await
                .map_err(map_llm_error)?;

            let generated_at_epoch_seconds = clock.now_epoch_seconds();
            Ok(WorkoutRecap::generated(
                response.message,
                response.provider.as_str(),
                response.model,
                generated_at_epoch_seconds,
            ))
        })
    }

    fn generate_initial_plan_window(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
    ) -> BoxFuture<Result<String, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let workout_recap = workout_recap.clone();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let workout_recap_json = json!({
                "text": workout_recap.text,
                "provider": workout_recap.provider,
                "model": workout_recap.model,
                "generatedAt": workout_recap.generated_at_epoch_seconds,
            })
            .to_string();
            let stable_context = format!(
                "saved_at_epoch_seconds={saved_at_epoch_seconds}\nworkout_recap={workout_recap_json}\ntraining_plan_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let user_prompt = "Generate the next 14 dated days starting the day after the completed workout. Return only dated sections in parser-friendly workout-builder text. Include rest days explicitly when needed.";

            let response = llm_chat_port
                .chat(
                    config,
                    LlmChatRequest {
                        user_id,
                        system_prompt: training_plan_initial_window_system_prompt(
                            context.context.profile.availability_configured,
                        ),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage {
                            role: LlmMessageRole::User,
                            content: user_prompt.to_string(),
                        }],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    },
                )
                .await
                .map_err(map_llm_error)?;

            Ok(response.message)
        })
    }

    fn correct_invalid_days(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
    ) -> BoxFuture<Result<String, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let workout_recap = workout_recap.clone();
        let invalid_day_sections = invalid_day_sections.to_string();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let workout_recap_json = json!({
                "text": workout_recap.text,
                "provider": workout_recap.provider,
                "model": workout_recap.model,
                "generatedAt": workout_recap.generated_at_epoch_seconds,
            })
            .to_string();
            let stable_context = format!(
                "saved_at_epoch_seconds={saved_at_epoch_seconds}\nworkout_recap={workout_recap_json}\ntraining_plan_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let issues_text = issues
                .iter()
                .map(|issue| format!("{}: {}", issue.scope, issue.message))
                .collect::<Vec<_>>()
                .join("\n");
            let user_prompt = format!(
                "Correct only these invalid dated sections. Keep valid days untouched.\n\nInvalid sections:\n{invalid_day_sections}\n\nValidation issues:\n{issues_text}"
            );

            let response = llm_chat_port
                .chat(
                    config,
                    LlmChatRequest {
                        user_id,
                        system_prompt: training_plan_correction_system_prompt(
                            context.context.profile.availability_configured,
                        ),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage {
                            role: LlmMessageRole::User,
                            content: user_prompt,
                        }],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    },
                )
                .await
                .map_err(map_llm_error)?;

            Ok(response.message)
        })
    }
}

fn map_llm_error(error: LlmError) -> TrainingPlanError {
    TrainingPlanError::Unavailable(error.to_string())
}

fn training_plan_recap_system_prompt() -> String {
    format!("{TRAINING_PLAN_RECAP_SYSTEM_PROMPT_BASE} {PACKED_TRAINING_CONTEXT_LEGEND}")
}

fn training_plan_initial_window_system_prompt(availability_configured: bool) -> String {
    format!(
        "{TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT_BASE} {} {TRAINING_PLAN_OUTPUT_GRAMMAR} {PACKED_TRAINING_CONTEXT_LEGEND}",
        training_plan_planning_guidelines(availability_configured),
    )
}

fn training_plan_correction_system_prompt(availability_configured: bool) -> String {
    format!(
        "{TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT_BASE} {} {TRAINING_PLAN_OUTPUT_GRAMMAR} {PACKED_TRAINING_CONTEXT_LEGEND}",
        training_plan_planning_guidelines(availability_configured),
    )
}

fn training_plan_planning_guidelines(availability_configured: bool) -> String {
    let availability_guidance = if availability_configured {
        TRAINING_PLAN_AVAILABILITY_CONFIGURED_GUIDANCE
    } else {
        TRAINING_PLAN_AVAILABILITY_UNCONFIGURED_GUIDANCE
    };

    format!("{TRAINING_PLAN_PLANNING_GUIDELINES_BASE} {availability_guidance}")
}

#[cfg(test)]
mod tests {
    use super::{
        training_plan_correction_system_prompt, training_plan_initial_window_system_prompt,
    };

    #[test]
    fn training_plan_prompts_include_durability_guidelines() {
        for prompt in [
            training_plan_initial_window_system_prompt(true),
            training_plan_correction_system_prompt(true),
        ] {
            assert!(
                prompt.contains("Metric hierarchy: RPE over power over TSS/TSB over heart rate.")
            );
            assert!(prompt.contains("Never prescribe more than 2 consecutive Rest Day entries unless the athlete explicitly reports illness or injury."));
            assert!(prompt.contains("Treat races as Category C by default unless the context explicitly says otherwise."));
            assert!(prompt.contains("Weekly availability is mandatory and must be respected"));
        }
    }

    #[test]
    fn training_plan_prompts_adjust_availability_guidance_when_not_configured() {
        for prompt in [
            training_plan_initial_window_system_prompt(false),
            training_plan_correction_system_prompt(false),
        ] {
            assert!(prompt.contains("Weekly availability is not configured in this context."));
            assert!(!prompt.contains("Weekly availability is mandatory and must be respected"));
        }
    }
}
