use super::super::{
    quality_prompt::truncate_evidence_section, PlanQualityEvidence, TrainingPlanGenerationOperation,
};
use crate::domain::llm::{LlmChatMessage, LlmMessageRole};

const TOOL_SIMULATE_FORWARD_LOAD: &str = "simulate_forward_load";
const TOOL_POWER_CURVE: &str = "selected_workout_power_curve";
const TOOL_W_PRIME: &str = "get_w_prime_balance";

pub(super) fn extract_plan_quality_evidence(
    operation: &TrainingPlanGenerationOperation,
) -> Option<PlanQualityEvidence> {
    let transcript = select_evidence_transcript(operation)?;
    Some(evidence_from_transcript(transcript))
}

fn select_evidence_transcript(
    operation: &TrainingPlanGenerationOperation,
) -> Option<&[LlmChatMessage]> {
    [
        &operation.correction_tool_loop_state,
        &operation.initial_plan_tool_loop_state,
    ]
    .into_iter()
    .flatten()
    .find(|state| !state.provider_transcript.is_empty())
    .map(|state| state.provider_transcript.as_slice())
}

fn evidence_from_transcript(transcript: &[LlmChatMessage]) -> PlanQualityEvidence {
    let mut load = None;
    let mut power_curve = None;
    let mut w_prime = None;
    let mut call_names_by_id = std::collections::HashMap::<&str, &str>::new();

    for message in transcript {
        for call in &message.tool_calls {
            call_names_by_id.insert(call.id.as_str(), call.name.as_str());
        }
        if message.role != LlmMessageRole::Tool {
            continue;
        }
        let Some(call_id) = message.tool_call_id.as_deref() else {
            continue;
        };
        let Some(name) = call_names_by_id.get(call_id).copied() else {
            continue;
        };
        match name {
            TOOL_SIMULATE_FORWARD_LOAD => load = Some(compact_forward_load(&message.content)),
            TOOL_POWER_CURVE => power_curve = Some(compact_power_curve(&message.content)),
            TOOL_W_PRIME => w_prime = Some(compact_w_prime(&message.content)),
            _ => {}
        }
    }

    PlanQualityEvidence {
        load,
        power_curve,
        w_prime,
    }
}

fn parse_evidence_json(content: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    if value.get("error").is_some() {
        None
    } else {
        Some(value)
    }
}

fn compact_forward_load(content: &str) -> String {
    let Some(value) = parse_evidence_json(content) else {
        return truncate_evidence_section(content);
    };

    let baseline = value.get("baseline");
    let baseline_bits = match baseline {
        Some(b) => format!(
            "baseline ctl={} atl={} tsb={}",
            json_num(b.get("ctl")),
            json_num(b.get("atl")),
            json_num(b.get("tsb")),
        ),
        None => "baseline=?".to_string(),
    };

    let days = value
        .get("days")
        .and_then(|d| d.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let mut min_tsb: Option<(f64, &str)> = None;
    let mut race_day: Option<(f64, &str)> = None;
    for day in days {
        let Some(date) = day.get("date").and_then(|d| d.as_str()) else {
            continue;
        };
        let Some(tsb) = day.get("tsb").and_then(|t| t.as_f64()) else {
            continue;
        };
        if min_tsb.is_none_or(|(current, _)| tsb < current) {
            min_tsb = Some((tsb, date));
        }
        let source = day.get("source").and_then(|s| s.as_str()).unwrap_or("");
        if source.contains("future_event") {
            race_day = Some((tsb, date));
        }
    }

    let min_bit = min_tsb
        .map(|(tsb, date)| format!("tsb_min={tsb}@{date}"))
        .unwrap_or_else(|| "tsb_min=?".to_string());
    let race_bit = race_day
        .map(|(tsb, date)| format!("race_day_tsb={tsb}@{date}"))
        .unwrap_or_else(|| "race_day_tsb=n/a".to_string());
    let trend_bit = match (days.first(), days.last()) {
        (Some(first), Some(last)) => format!(
            "ctl {}→{} atl {}→{}",
            json_num(first.get("ctl")),
            json_num(last.get("ctl")),
            json_num(first.get("atl")),
            json_num(last.get("atl")),
        ),
        _ => "ctl/atl trend=n/a".to_string(),
    };

    truncate_evidence_section(&format!(
        "{baseline_bits}; {min_bit}; {race_bit}; {trend_bit}"
    ))
}

fn compact_power_curve(content: &str) -> String {
    let Some(value) = parse_evidence_json(content) else {
        return truncate_evidence_section(content);
    };

    let date = value.get("date").and_then(|d| d.as_str()).unwrap_or("?");
    let points = value
        .get("max_average_watts")
        .and_then(|w| w.as_array())
        .map(|points| {
            let step = value
                .get("duration_step_seconds")
                .and_then(|s| s.as_i64())
                .unwrap_or(5);
            let start = value
                .get("duration_start_seconds")
                .and_then(|s| s.as_i64())
                .unwrap_or(step);
            let last = points.len().saturating_sub(1);
            let mut idxs = vec![0usize, last / 4, last / 2, (last * 3) / 4, last];
            idxs.sort_unstable();
            idxs.dedup();
            let sample: Vec<String> = idxs
                .into_iter()
                .filter(|&idx| idx < points.len())
                .filter_map(|idx| {
                    let watts = points[idx]
                        .as_f64()
                        .or_else(|| points[idx].as_i64().map(|v| v as f64))?;
                    let duration = start + (idx as i64) * step;
                    Some(format!("{duration}s={watts}"))
                })
                .collect();
            if sample.is_empty() {
                "points=[]".to_string()
            } else {
                format!("points=[{}]", sample.join(","))
            }
        })
        .unwrap_or_else(|| "points=?".to_string());

    truncate_evidence_section(&format!("date={date}; {points}"))
}

fn compact_w_prime(content: &str) -> String {
    let Some(value) = parse_evidence_json(content) else {
        return truncate_evidence_section(content);
    };

    let date = value.get("date").and_then(|d| d.as_str()).unwrap_or("?");
    let summary = value.get("summary");
    let min_balance = json_num(summary.and_then(|s| s.get("min_w_prime_balance")));
    let depleted = summary
        .and_then(|s| s.get("w_prime_depleted"))
        .and_then(|d| d.as_bool())
        .map(|d| d.to_string())
        .unwrap_or_else(|| "?".to_string());

    truncate_evidence_section(&format!(
        "date={date}; cp={}; w_prime={}; min_balance={min_balance}; depleted={depleted}",
        json_num(value.get("cp_watts")),
        json_num(value.get("w_prime_joules")),
    ))
}

fn json_num(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => "?".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::llm::{LlmChatMessage, LlmToolCall};
    use crate::domain::llm_tools::LlmToolLoopState;
    use crate::domain::training_plan::TrainingPlanGenerationOperation;

    use super::{evidence_from_transcript, extract_plan_quality_evidence};

    fn sample_operation() -> TrainingPlanGenerationOperation {
        TrainingPlanGenerationOperation::pending(
            "training-plan:u:w:1".to_string(),
            "u".to_string(),
            "w".to_string(),
            100,
            200,
        )
    }

    fn tool_pair(id: &str, name: &str, content: &str) -> (LlmChatMessage, LlmChatMessage) {
        (
            LlmChatMessage::assistant_with_tool_calls(
                "",
                vec![LlmToolCall {
                    id: id.to_string(),
                    name: name.to_string(),
                    arguments_json: "{}".to_string(),
                }],
            ),
            LlmChatMessage::tool(id, content),
        )
    }

    #[test]
    fn extract_returns_none_without_transcripts() {
        assert!(extract_plan_quality_evidence(&sample_operation()).is_none());
    }

    #[test]
    fn extract_prefers_correction_transcript_and_lists_missing() {
        let mut operation = sample_operation();
        let (assistant, tool) = tool_pair(
            "c1",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-05-10","ctl":51,"atl":45,"tsb":6,"source":"input"},{"date":"2026-05-12","ctl":52,"atl":48,"tsb":-2,"source":"future_event"}]}"#,
        );
        operation.correction_tool_loop_state = Some(LlmToolLoopState {
            provider_transcript: vec![assistant, tool],
            ..Default::default()
        });
        operation.initial_plan_tool_loop_state = Some(LlmToolLoopState {
            provider_transcript: vec![LlmChatMessage::user("ignore")],
            ..Default::default()
        });

        let evidence = extract_plan_quality_evidence(&operation).unwrap();
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("tsb_min=-2@2026-05-12"));
        assert!(load.contains("race_day_tsb=-2@2026-05-12"));
        assert!(evidence.power_curve.is_none());
        assert!(evidence.w_prime.is_none());
    }

    #[test]
    fn evidence_from_transcript_compacts_all_three_tools() {
        let (a1, t1) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":1,"atl":2,"tsb":3},"days":[{"date":"2026-01-01","ctl":1,"atl":2,"tsb":3,"source":"input"}]}"#,
        );
        let (a2, t2) = tool_pair(
            "2",
            "selected_workout_power_curve",
            r#"{"date":"2026-01-01","duration_start_seconds":5,"duration_step_seconds":5,"max_average_watts":[400,350,300]}"#,
        );
        let (a3, t3) = tool_pair(
            "3",
            "get_w_prime_balance",
            r#"{"date":"2026-01-01","cp_watts":270,"w_prime_joules":22000,"summary":{"min_w_prime_balance":1200.5,"w_prime_depleted":false}}"#,
        );
        let evidence = evidence_from_transcript(&[a1, t1, a2, t2, a3, t3]);
        assert!(evidence.load.is_some());
        assert!(evidence
            .power_curve
            .as_ref()
            .unwrap()
            .contains("date=2026-01-01"));
        assert!(evidence
            .w_prime
            .as_ref()
            .unwrap()
            .contains("min_balance=1200.5"));
    }

    #[test]
    fn evidence_from_transcript_accepts_raw_non_json_payload() {
        let (assistant, tool) = tool_pair("x", "simulate_forward_load", "not-json-but-useful");
        let evidence = evidence_from_transcript(&[assistant, tool]);
        assert_eq!(evidence.load.as_deref(), Some("not-json-but-useful"));
    }
}
