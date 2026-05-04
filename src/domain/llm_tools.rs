use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{
    intervals::{parse_planned_workout_days, parse_workout_doc, serialize_planned_workout},
    llm::{
        merge_provider_transcript_entries, LlmChatMessage, LlmChatPort, LlmChatRequest,
        LlmChatResponse, LlmError, LlmFinishReason, LlmProvider, LlmProviderConfig, LlmToolChoice,
        LlmToolDefinition,
    },
    training_context::{FuturePlannedEventContext, ProjectedDayContext, TrainingContext},
    workout_summary::PublicToolCall,
};

pub const TOOL_LOOP_MAX_ROUNDS: u32 = 6;
const SIMULATE_FORWARD_LOAD_TOOL_NAME: &str = "simulate_forward_load";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolScope {
    WorkoutSummaryChat,
    CalendarCoachChat,
    TrainingPlanGeneration,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LlmToolLoopState {
    pub provider_transcript: Vec<LlmChatMessage>,
    pub finish_reason: Option<LlmFinishReason>,
    pub public_tool_calls: Vec<PublicToolCall>,
    pub round_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmToolLoopOutput {
    pub response: LlmChatResponse,
    pub state: LlmToolLoopState,
}

impl LlmToolLoopOutput {
    pub fn from_response(response: LlmChatResponse) -> Self {
        let message = response.message.clone();
        let finish_reason = response.finish_reason.clone();

        Self {
            response,
            state: LlmToolLoopState {
                provider_transcript: vec![message],
                finish_reason,
                public_tool_calls: Vec::new(),
                round_count: 1,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolExecutionContext {
    pub training_context: TrainingContext,
    pub today: String,
}

type BoxToolFuture = Pin<Box<dyn Future<Output = Result<LlmToolLoopOutput, LlmError>> + Send>>;

/// Future returned by a tool-loop checkpoint callback.
pub type ToolLoopCheckpointFuture = Pin<Box<dyn Future<Output = Result<(), LlmError>> + Send>>;

/// Callback invoked after each tool round in `run_tool_loop_with_checkpoint`.
/// Receives the current loop state so callers can persist resumable progress.
pub type ToolLoopCheckpoint =
    Arc<dyn Fn(LlmToolLoopState) -> ToolLoopCheckpointFuture + Send + Sync>;

type PlannedLoadEstimate = (f64, Option<i32>, String, bool, Option<String>);

/// Run a tool loop without per-round checkpointing.
/// Equivalent to `run_tool_loop_with_checkpoint(..., None)`.
pub fn run_tool_loop(
    chat_port: std::sync::Arc<dyn LlmChatPort>,
    config: LlmProviderConfig,
    request: LlmChatRequest,
    scope: ToolScope,
    tool_context: ToolExecutionContext,
    restored_state: Option<LlmToolLoopState>,
) -> BoxToolFuture {
    run_tool_loop_with_checkpoint(
        chat_port,
        config,
        request,
        scope,
        tool_context,
        restored_state,
        None,
    )
}

/// Run a tool loop with optional per-round checkpointing.
///
/// If `checkpoint` is provided, it is invoked after each tool round (after tool
/// results have been appended to the conversation) so callers can persist
/// resumable state. If the checkpoint returns an error the loop aborts and
/// returns that error immediately.
pub fn run_tool_loop_with_checkpoint(
    chat_port: std::sync::Arc<dyn LlmChatPort>,
    config: LlmProviderConfig,
    mut request: LlmChatRequest,
    scope: ToolScope,
    tool_context: ToolExecutionContext,
    restored_state: Option<LlmToolLoopState>,
    checkpoint: Option<ToolLoopCheckpoint>,
) -> BoxToolFuture {
    Box::pin(async move {
        let mut conversation = request.conversation;
        let mut state = restored_state.unwrap_or_default();
        if !state.provider_transcript.is_empty() {
            conversation.extend(state.provider_transcript.clone());
        }

        let tools = tool_definitions_for_scope(scope, &config.provider);
        let tool_choice = if tools.is_empty() {
            LlmToolChoice::None
        } else {
            LlmToolChoice::Auto
        };

        for _ in state.round_count..TOOL_LOOP_MAX_ROUNDS {
            request.conversation = conversation.clone();
            request.tools = tools.clone();
            request.tool_choice = tool_choice.clone();

            let response = chat_port.chat(config.clone(), request.clone()).await?;
            let new_public_tool_calls = response
                .tool_calls()
                .iter()
                .map(public_tool_call_from_llm)
                .collect::<Vec<_>>();

            let mut provider_transcript = merge_provider_transcript_entries(
                state.provider_transcript.clone(),
                std::slice::from_ref(&response.message),
            );
            let public_tool_calls =
                merge_public_tool_calls(state.public_tool_calls.clone(), &new_public_tool_calls);
            let round_count = state.round_count.saturating_add(1);

            conversation.push(response.message.clone());

            if response.tool_calls().is_empty() {
                return Ok(LlmToolLoopOutput {
                    response: response.clone(),
                    state: LlmToolLoopState {
                        provider_transcript,
                        finish_reason: response.finish_reason,
                        public_tool_calls,
                        round_count,
                    },
                });
            }

            for tool_call in response.tool_calls() {
                let tool_message = LlmChatMessage::tool(
                    tool_call.id.clone(),
                    execute_tool_call(
                        tool_call.name.as_str(),
                        tool_call.arguments_json.as_str(),
                        &tool_context,
                    ),
                );
                provider_transcript.push(tool_message.clone());
                conversation.push(tool_message);
            }

            state = LlmToolLoopState {
                provider_transcript,
                finish_reason: response.finish_reason.clone(),
                public_tool_calls,
                round_count,
            };

            if let Some(checkpoint) = checkpoint.as_ref() {
                checkpoint(state.clone()).await?;
            }
        }

        Err(LlmError::InvalidResponse(format!(
            "tool loop exceeded {TOOL_LOOP_MAX_ROUNDS} rounds"
        )))
    })
}

pub fn tool_definitions_for_scope(
    scope: ToolScope,
    provider: &LlmProvider,
) -> Vec<LlmToolDefinition> {
    if !provider_supports_tools(provider) {
        return Vec::new();
    }

    match scope {
        ToolScope::WorkoutSummaryChat
        | ToolScope::CalendarCoachChat
        | ToolScope::TrainingPlanGeneration => vec![LlmToolDefinition {
            name: SIMULATE_FORWARD_LOAD_TOOL_NAME.to_string(),
            description: "Simulate 14 days of forward training load from today using dated workout text and return per-day CTL ATL TSB and planned load estimates.".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dated_workout_text": {
                        "type": "string",
                        "description": "Raw dated workout text in the existing YYYY-MM-DD plus workout-builder format."
                    }
                },
                "required": ["dated_workout_text"]
            })
            .to_string(),
        }],
    }
}

fn provider_supports_tools(provider: &LlmProvider) -> bool {
    matches!(provider, LlmProvider::OpenAi | LlmProvider::OpenRouter)
}

pub fn public_tool_call_from_llm(tool_call: &crate::domain::llm::LlmToolCall) -> PublicToolCall {
    PublicToolCall {
        id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        arguments_json: tool_call.arguments_json.clone(),
        arguments_preview: preview_tool_arguments(&tool_call.name, &tool_call.arguments_json),
    }
}

fn merge_public_tool_calls(
    mut existing: Vec<PublicToolCall>,
    pending: &[PublicToolCall],
) -> Vec<PublicToolCall> {
    for tool_call in pending {
        if existing
            .iter()
            .any(|existing_call| existing_call.id == tool_call.id)
        {
            continue;
        }
        existing.push(tool_call.clone());
    }

    existing
}

fn preview_tool_arguments(tool_name: &str, arguments_json: &str) -> Option<String> {
    match tool_name {
        SIMULATE_FORWARD_LOAD_TOOL_NAME => {
            let args: SimulateForwardLoadArgs = serde_json::from_str(arguments_json).ok()?;
            let parsed = parse_planned_workout_days(&args.dated_workout_text).ok()?;
            let first = parsed.days.first()?.date.clone();
            let last = parsed.days.last()?.date.clone();
            Some(format!(
                "{} dated days from {first} to {last}",
                parsed.days.len()
            ))
        }
        _ => None,
    }
}

fn execute_tool_call(
    tool_name: &str,
    arguments_json: &str,
    context: &ToolExecutionContext,
) -> String {
    match tool_name {
        SIMULATE_FORWARD_LOAD_TOOL_NAME => simulate_forward_load(arguments_json, context),
        _ => json!({
            "error": format!("unknown tool: {tool_name}")
        })
        .to_string(),
    }
}

#[derive(Deserialize)]
struct SimulateForwardLoadArgs {
    dated_workout_text: String,
}

#[derive(Serialize)]
struct SimulateForwardLoadResponse {
    baseline: ForwardLoadBaseline,
    days: Vec<ForwardLoadDay>,
}

#[derive(Serialize)]
struct ForwardLoadBaseline {
    today: String,
    ctl: f64,
    atl: f64,
    tsb: f64,
    ftp_watts: Option<i32>,
}

#[derive(Serialize)]
struct ForwardLoadDay {
    date: String,
    planned_tss: f64,
    planned_duration_seconds: Option<i32>,
    source: String,
    rest_day: bool,
    rest_day_reason: Option<String>,
    ctl: f64,
    atl: f64,
    tsb: f64,
}

fn simulate_forward_load(arguments_json: &str, context: &ToolExecutionContext) -> String {
    let args = match serde_json::from_str::<SimulateForwardLoadArgs>(arguments_json) {
        Ok(args) => args,
        Err(error) => {
            return json!({
                "error": format!("invalid simulate_forward_load arguments: {error}")
            })
            .to_string();
        }
    };

    let parsed_days = match parse_planned_workout_days(&args.dated_workout_text) {
        Ok(parsed) => parsed,
        Err(error) => {
            return json!({
                "error": format!("failed to parse dated workout text: {error}")
            })
            .to_string();
        }
    };

    let Some(today) = parse_date(&context.today) else {
        return json!({
            "error": format!("invalid today date in tool context: {}", context.today)
        })
        .to_string();
    };

    let ftp_watts = context.training_context.history.ftp_current;
    let (mut ctl, mut atl) = resolve_load_baseline(&context.training_context);
    let mut days = Vec::with_capacity(14);

    for offset in 1..=14 {
        let date = today + Duration::days(offset);
        let date_key = format_date(date);
        let planned = input_day_estimate(&parsed_days.days, &date_key, ftp_watts)
            .or_else(|| {
                projected_day_estimate(
                    &context.training_context.projected_days,
                    &date_key,
                    ftp_watts,
                )
            })
            .or_else(|| future_event_estimate(&context.training_context.future_events, &date_key));
        let (planned_tss, planned_duration_seconds, source, rest_day, rest_day_reason) = planned
            .unwrap_or((
                0.0,
                None,
                "empty".to_string(),
                true,
                Some("no planned load".to_string()),
            ));

        ctl = update_load(ctl, planned_tss, 42.0);
        atl = update_load(atl, planned_tss, 7.0);
        let tsb = round_to_2(ctl - atl);

        days.push(ForwardLoadDay {
            date: date_key,
            planned_tss: round_to_2(planned_tss),
            planned_duration_seconds,
            source,
            rest_day,
            rest_day_reason,
            ctl: round_to_2(ctl),
            atl: round_to_2(atl),
            tsb,
        });
    }

    json!(SimulateForwardLoadResponse {
        baseline: ForwardLoadBaseline {
            today: context.today.clone(),
            ctl: round_to_2(ctl_from_context(&context.training_context)),
            atl: round_to_2(atl_from_context(&context.training_context)),
            tsb: round_to_2(tsb_from_context(&context.training_context)),
            ftp_watts,
        },
        days,
    })
    .to_string()
}

fn input_day_estimate(
    days: &[crate::domain::intervals::PlannedWorkoutDay],
    date: &str,
    ftp_watts: Option<i32>,
) -> Option<PlannedLoadEstimate> {
    let day = days.iter().find(|day| day.date == date)?;
    if day.is_rest_day() {
        return Some((
            0.0,
            None,
            "input".to_string(),
            true,
            day.rest_day_reason().map(str::to_string),
        ));
    }

    let workout = day.planned_workout()?;
    let raw = serialize_planned_workout(workout);
    let parsed = parse_workout_doc(Some(raw.as_str()), ftp_watts);

    Some((
        parsed
            .summary
            .estimated_training_stress_score
            .unwrap_or(0.0),
        Some(parsed.summary.total_duration_seconds),
        "input".to_string(),
        false,
        None,
    ))
}

fn projected_day_estimate(
    projected_days: &[ProjectedDayContext],
    date: &str,
    ftp_watts: Option<i32>,
) -> Option<PlannedLoadEstimate> {
    let projected = projected_days
        .iter()
        .find(|projected| projected.date == date)?;
    let workout = projected.workouts.first()?;
    if workout.rest_day {
        return Some((
            0.0,
            None,
            "projected".to_string(),
            true,
            workout.rest_day_reason.clone(),
        ));
    }
    let raw = workout.raw_workout_doc.as_deref()?;
    let parsed = parse_workout_doc(Some(raw), ftp_watts);
    Some((
        parsed
            .summary
            .estimated_training_stress_score
            .unwrap_or(0.0),
        Some(parsed.summary.total_duration_seconds),
        "projected".to_string(),
        false,
        None,
    ))
}

fn future_event_estimate(
    future_events: &[FuturePlannedEventContext],
    date: &str,
) -> Option<PlannedLoadEstimate> {
    let event = future_events
        .iter()
        .find(|event| event.start_date_local.get(..10) == Some(date))?;

    Some((
        event.estimated_training_stress_score.unwrap_or(0.0),
        event.estimated_duration_seconds,
        "future_event".to_string(),
        false,
        None,
    ))
}

fn resolve_load_baseline(training_context: &TrainingContext) -> (f64, f64) {
    (
        ctl_from_context(training_context),
        atl_from_context(training_context),
    )
}

fn ctl_from_context(training_context: &TrainingContext) -> f64 {
    training_context
        .history
        .ctl
        .or_else(|| {
            training_context
                .history
                .load_trend
                .last()
                .and_then(|point| point.ctl)
        })
        .unwrap_or_default()
}

fn atl_from_context(training_context: &TrainingContext) -> f64 {
    training_context
        .history
        .atl
        .or_else(|| {
            training_context
                .history
                .load_trend
                .last()
                .and_then(|point| point.atl)
        })
        .or_else(|| {
            training_context
                .history
                .tsb
                .map(|tsb| ctl_from_context(training_context) - tsb)
        })
        .unwrap_or_default()
}

fn tsb_from_context(training_context: &TrainingContext) -> f64 {
    training_context
        .history
        .tsb
        .unwrap_or_else(|| ctl_from_context(training_context) - atl_from_context(training_context))
}

fn update_load(current: f64, planned_tss: f64, time_constant_days: f64) -> f64 {
    current + (planned_tss - current) * (1.0 / time_constant_days)
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn format_date(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::{preview_tool_arguments, simulate_forward_load, ToolExecutionContext};
    use crate::domain::training_context::{
        AthleteProfileContext, ProjectedDayContext, ProjectedWorkoutContext, TrainingContext,
    };

    fn sample_context() -> ToolExecutionContext {
        let mut training_context = TrainingContext {
            generated_at_epoch_seconds: 1_700_000_000,
            focus_workout_id: None,
            focus_kind: "summary".to_string(),
            intervals_status: Default::default(),
            profile: AthleteProfileContext {
                availability_configured: true,
                ..Default::default()
            },
            races: Vec::new(),
            future_events: Vec::new(),
            history: Default::default(),
            recent_days: Vec::new(),
            upcoming_days: Vec::new(),
            projected_days: vec![
                ProjectedDayContext {
                    date: "2026-05-06".to_string(),
                    workouts: vec![ProjectedWorkoutContext {
                        source_workout_id: "planned-1".to_string(),
                        start_date_local: "2026-05-06T06:00:00".to_string(),
                        name: Some("Projected Endurance".to_string()),
                        interval_blocks: Vec::new(),
                        raw_workout_doc: Some("- 90m 65%".to_string()),
                        rest_day: false,
                        rest_day_reason: None,
                    }],
                },
                ProjectedDayContext {
                    date: "2026-05-07".to_string(),
                    workouts: vec![ProjectedWorkoutContext {
                        source_workout_id: "planned-2".to_string(),
                        start_date_local: "2026-05-07T06:00:00".to_string(),
                        name: Some("Projected Tempo".to_string()),
                        interval_blocks: Vec::new(),
                        raw_workout_doc: Some("- 60m 75%".to_string()),
                        rest_day: false,
                        rest_day_reason: None,
                    }],
                },
            ],
        };
        training_context.history.ctl = Some(72.0);
        training_context.history.atl = Some(81.0);
        training_context.history.tsb = Some(-9.0);
        training_context.history.ftp_current = Some(300);

        ToolExecutionContext {
            training_context,
            today: "2026-05-04".to_string(),
        }
    }

    #[test]
    fn preview_tool_arguments_summarizes_dated_text_range() {
        let preview = preview_tool_arguments(
            "simulate_forward_load",
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day"}"#,
        );

        assert_eq!(
            preview.as_deref(),
            Some("2 dated days from 2026-05-05 to 2026-05-06")
        );
    }

    #[test]
    fn simulate_forward_load_returns_14_day_sequence() {
        let response = simulate_forward_load(
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day: absorb load"}"#,
            &sample_context(),
        );

        assert!(response.contains("\"baseline\""));
        assert!(response.contains("\"2026-05-05\""));
        assert!(response.contains("\"source\":\"input\""));
        assert!(response.contains("\"source\":\"projected\""));
        assert!(response.contains("\"2026-05-18\""));
    }

    #[test]
    fn tool_loop_state_defaults_empty() {
        let state = super::LlmToolLoopState::default();
        assert!(state.provider_transcript.is_empty());
        assert!(state.public_tool_calls.is_empty());
        assert_eq!(state.round_count, 0);
    }
}
