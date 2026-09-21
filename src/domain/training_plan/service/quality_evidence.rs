use super::super::{
    quality_prompt::{truncate_evidence_section, EVIDENCE_SECTION_MAX_CHARS},
    PlanQualityEvidence, TrainingPlanGenerationOperation,
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

fn is_race_day_load_source(source: &str) -> bool {
    source
        .split('+')
        .any(|part| part == "future_event" || part == "race")
}

struct ForwardLoadRaceDay<'a> {
    tsb: f64,
    date: &'a str,
    tss_source: &'a str,
    planned_tss: Option<f64>,
    pair_idx: usize,
}

struct ForwardLoadDayScan<'a> {
    pairs: Vec<String>,
    min_tsb: Option<(f64, &'a str)>,
    min_pair_idx: Option<usize>,
    race_day: Option<ForwardLoadRaceDay<'a>>,
    today_in_days: bool,
}

fn compact_forward_load(content: &str) -> String {
    let Some(value) = parse_evidence_json(content) else {
        return truncate_evidence_section(content);
    };

    let baseline = value.get("baseline");
    let baseline_today = baseline.and_then(|b| b.get("today").and_then(|t| t.as_str()));
    let baseline_applied = value.get("baseline_applied_load");
    let baseline_bits = format_forward_load_baseline(baseline, baseline_today);
    let days = value
        .get("days")
        .and_then(|d| d.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let scan = scan_forward_load_days(days, baseline_today);

    let min_bit = scan
        .min_tsb
        .map(|(tsb, date)| format!("tsb_min={tsb}@{date}"))
        .unwrap_or_else(|| "tsb_min=?".to_string());
    let race_bit = format_forward_load_race_bit(baseline, baseline_today, &scan, baseline_applied);
    let race_tss_bit = match (baseline_applied, &scan.race_day) {
        (Some(_), _) => String::new(),
        (None, Some(race)) => {
            let tss = race
                .planned_tss
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string());
            format!("race_tss={tss} (source={})", race.tss_source)
        }
        (None, None) => String::new(),
    };
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
    let notes_bit = format_forward_load_notes(value.get("notes"));

    let head = if race_tss_bit.is_empty() {
        format!("{baseline_bits}; {min_bit}; {race_bit}; {trend_bit}")
    } else {
        format!("{baseline_bits}; {min_bit}; {race_bit}; {race_tss_bit}; {trend_bit}")
    };

    let days_bit = if scan.pairs.is_empty() {
        String::new()
    } else {
        let reserved =
            head.chars().count() + notes_bit.chars().count() + "; days=[]; ".chars().count();
        let days_budget = EVIDENCE_SECTION_MAX_CHARS.saturating_sub(reserved).max(16);
        let mut protected = std::collections::BTreeSet::new();
        protected.insert(0);
        protected.insert(scan.pairs.len() - 1);
        if let Some(idx) = scan.min_pair_idx {
            protected.insert(idx);
        }
        if let Some(race) = &scan.race_day {
            protected.insert(race.pair_idx);
        }
        format!(
            "; days=[{}]",
            trim_forward_load_day_pairs(scan.pairs, &protected, days_budget)
        )
    };

    truncate_evidence_section(&format!("{head}{days_bit}{notes_bit}"))
}

fn format_forward_load_baseline(
    baseline: Option<&serde_json::Value>,
    baseline_today: Option<&str>,
) -> String {
    let Some(b) = baseline else {
        return "baseline=?".to_string();
    };
    let metrics = format!(
        "ctl={} atl={} tsb={}",
        json_num(b.get("ctl")),
        json_num(b.get("atl")),
        json_num(b.get("tsb")),
    );
    match baseline_today {
        Some(today) => format!("baseline({today}) {metrics}"),
        None => format!("baseline {metrics}"),
    }
}

fn scan_forward_load_days<'a>(
    days: &'a [serde_json::Value],
    baseline_today: Option<&str>,
) -> ForwardLoadDayScan<'a> {
    let mut scan = ForwardLoadDayScan {
        pairs: Vec::new(),
        min_tsb: None,
        min_pair_idx: None,
        race_day: None,
        today_in_days: false,
    };
    for day in days {
        let Some(date) = day.get("date").and_then(|d| d.as_str()) else {
            continue;
        };
        if baseline_today == Some(date) {
            scan.today_in_days = true;
        }
        let Some(tsb) = day.get("tsb").and_then(|t| t.as_f64()) else {
            continue;
        };
        if scan.min_tsb.is_none_or(|(current, _)| tsb < current) {
            scan.min_tsb = Some((tsb, date));
            scan.min_pair_idx = Some(scan.pairs.len());
        }
        let source = day.get("source").and_then(|s| s.as_str()).unwrap_or("");
        if is_race_day_load_source(source) {
            scan.race_day = Some(ForwardLoadRaceDay {
                tsb,
                date,
                tss_source: day
                    .get("tss_source")
                    .and_then(|s| s.as_str())
                    .unwrap_or("none"),
                planned_tss: day.get("planned_tss").and_then(|t| t.as_f64()),
                pair_idx: scan.pairs.len(),
            });
        }
        scan.pairs
            .push(format_forward_load_day_pair(day, date, tsb));
    }
    scan
}

fn format_forward_load_race_bit(
    baseline: Option<&serde_json::Value>,
    baseline_today: Option<&str>,
    scan: &ForwardLoadDayScan<'_>,
    baseline_applied: Option<&serde_json::Value>,
) -> String {
    if let Some(applied) = baseline_applied {
        let date = applied.get("date").and_then(|d| d.as_str()).unwrap_or("?");
        let tss = json_num(applied.get("tss"));
        let tsb = json_num(baseline.and_then(|b| b.get("tsb")));
        return format!(
            "race_day_tsb={tsb}@{date} (race_day_tsb_source=baseline_with_estimated_race_load, race_tss={tss})"
        );
    }
    match &scan.race_day {
        Some(race) if race.tss_source == "none" => {
            format!(
                "race_day_tsb=unavailable@{} (race_day_tsb_source=none)",
                race.date
            )
        }
        Some(race) if race.tss_source == "estimated" => {
            format!(
                "race_day_tsb={}@{} (race_day_tsb_source=estimated)",
                race.tsb, race.date
            )
        }
        Some(race) => format!(
            "race_day_tsb={}@{} (race_day_tsb_source=event_tss)",
            race.tsb, race.date
        ),
        None => match (baseline, baseline_today) {
            (Some(b), Some(today)) if !scan.today_in_days => format!(
                "race_day_tsb={}@{today} (race_day_tsb_source=baseline_pre_window, race load not included)",
                json_num(b.get("tsb")),
            ),
            _ => "race_day_tsb=n/a".to_string(),
        },
    }
}

fn format_forward_load_notes(notes: Option<&serde_json::Value>) -> String {
    notes
        .and_then(|n| n.as_array())
        .filter(|notes| !notes.is_empty())
        .map(|notes| {
            let joined = notes
                .iter()
                .filter_map(|n| n.as_str())
                .take(2)
                .collect::<Vec<_>>()
                .join("; ");
            format!("; notes=[{joined}]")
        })
        .unwrap_or_default()
}

fn format_forward_load_day_pair(day: &serde_json::Value, date: &str, tsb: f64) -> String {
    let mut pair = format!("{date}:{tsb}");
    if day
        .get("rest_day")
        .and_then(|r| r.as_bool())
        .unwrap_or(false)
    {
        pair.push_str("(rest)");
    }
    if let Some(tss) = day.get("planned_tss").and_then(|t| t.as_f64()) {
        let src = day
            .get("tss_source")
            .and_then(|s| s.as_str())
            .unwrap_or("?");
        pair.push_str(&format!("@tss={tss}:src={src}"));
    }
    pair
}

fn trim_forward_load_day_pairs(
    pairs: Vec<String>,
    protected: &std::collections::BTreeSet<usize>,
    max_chars: usize,
) -> String {
    const MARKER: &str = "…[trimmed]";
    let join_kept = |keep: &[bool]| -> String {
        pairs
            .iter()
            .enumerate()
            .filter(|(idx, _)| keep[*idx])
            .map(|(_, pair)| pair.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut keep = vec![true; pairs.len()];
    let body = join_kept(&keep);
    if body.chars().count() <= max_chars {
        return body;
    }

    let marker_overhead = 2 + MARKER.chars().count();
    let protected_count = protected.iter().filter(|&&idx| idx < pairs.len()).count();
    while keep.iter().filter(|&&k| k).count() > protected_count
        && join_kept(&keep).chars().count() + marker_overhead > max_chars
    {
        let kept_idxs: Vec<usize> = keep
            .iter()
            .enumerate()
            .filter(|(idx, k)| **k && !protected.contains(idx))
            .map(|(idx, _)| idx)
            .collect();
        let mid = pairs.len() / 2;
        let Some(&remove_at) = kept_idxs.iter().min_by_key(|idx| idx.abs_diff(mid)) else {
            break;
        };
        keep[remove_at] = false;
    }

    let body = join_kept(&keep);
    let trimmed = keep.iter().any(|&k| !k);
    if trimmed {
        let with_marker = format!("{body}, {MARKER}");
        if with_marker.chars().count() <= max_chars {
            return with_marker;
        }
        return with_marker.chars().take(max_chars).collect();
    }
    if body.chars().count() <= max_chars {
        return body;
    }
    body.chars().take(max_chars).collect()
}

fn compact_insufficient_data(value: &serde_json::Value) -> Option<String> {
    let status = value.get("status").and_then(|s| s.as_str())?;
    if status != "insufficient_data" {
        return None;
    }
    let reason = value
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("unknown");
    let date = value.get("date").and_then(|d| d.as_str()).unwrap_or("?");
    Some(format!(
        "status=insufficient_data; date={date}; reason={reason}"
    ))
}

fn compact_power_curve(content: &str) -> String {
    let Some(value) = parse_evidence_json(content) else {
        return truncate_evidence_section(content);
    };

    if let Some(insufficient) = compact_insufficient_data(&value) {
        return truncate_evidence_section(&insufficient);
    }

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

    if let Some(insufficient) = compact_insufficient_data(&value) {
        return truncate_evidence_section(&insufficient);
    }

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
            r#"{"baseline":{"ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-05-10","ctl":51,"atl":45,"tsb":6,"source":"input"},{"date":"2026-05-12","ctl":52,"atl":48,"tsb":-2,"source":"future_event","tss_source":"planned"}]}"#,
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
        assert!(load.contains("race_day_tsb=-2@2026-05-12 (race_day_tsb_source=event_tss)"));
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
    fn extract_uses_initial_after_replan_clears_stale_correction() {
        let mut operation = sample_operation();
        let (stale_assistant, stale_tool) = tool_pair(
            "stale",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":1,"atl":1,"tsb":1},"days":[{"date":"2026-01-01","ctl":1,"atl":1,"tsb":-99,"source":"future_event"}]}"#,
        );
        operation.correction_tool_loop_state = Some(LlmToolLoopState {
            provider_transcript: vec![stale_assistant, stale_tool],
            ..Default::default()
        });
        let (fresh_assistant, fresh_tool) = tool_pair(
            "fresh",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":10,"atl":5,"tsb":5},"days":[{"date":"2026-02-01","ctl":11,"atl":6,"tsb":5,"source":"input"}]}"#,
        );
        operation = operation.with_raw_plan_payload(
            "new-draft".to_string(),
            None,
            LlmToolLoopState {
                provider_transcript: vec![fresh_assistant, fresh_tool],
                ..Default::default()
            },
            300,
        );

        let evidence = extract_plan_quality_evidence(&operation).unwrap();
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("tsb_min=5@2026-02-01"));
        assert!(!load.contains("tsb_min=-99"));
        assert!(operation.correction_tool_loop_state.is_none());
    }

    #[test]
    fn evidence_from_transcript_accepts_raw_non_json_payload() {
        let (assistant, tool) = tool_pair("x", "simulate_forward_load", "not-json-but-useful");
        let evidence = evidence_from_transcript(&[assistant, tool]);
        assert_eq!(evidence.load.as_deref(), Some("not-json-but-useful"));
    }

    #[test]
    fn compact_marks_unknown_race_tss_as_unavailable() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-05-12","ctl":52,"atl":48,"tsb":-2,"source":"future_event","tss_source":"none"}],"notes":["2026-05-12: race TSS unknown; TSB shown assumes zero race load"]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("race_day_tsb=unavailable@2026-05-12 (race_day_tsb_source=none)"));
        assert!(load.contains("race TSS unknown"));
    }

    #[test]
    fn compact_labels_estimated_race_tss_source() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-05-12","ctl":52,"atl":48,"tsb":-2,"source":"future_event","tss_source":"estimated"}],"notes":["2026-05-12: race TSS estimated (tss=128.00, duration_s=7200); not measured"]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("race_day_tsb=-2@2026-05-12 (race_day_tsb_source=estimated)"));
        assert!(load.contains("race TSS estimated"));
    }

    #[test]
    fn compact_labels_estimated_tss_from_race_calendar_source() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-09-20","ctl":52,"atl":48,"tsb":3,"source":"race","tss_source":"estimated","planned_tss":128}],"notes":["2026-09-20: race TSS estimated (tss=128.00, duration_s=7200); not measured"]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("race_day_tsb=3@2026-09-20 (race_day_tsb_source=estimated)"));
        assert!(load.contains("race_tss=128 (source=estimated)"));
    }

    #[test]
    fn compact_labels_event_tss_race_source() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-05-12","ctl":52,"atl":48,"tsb":4,"source":"future_event","tss_source":"planned"}]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("race_day_tsb=4@2026-05-12 (race_day_tsb_source=event_tss)"));
    }

    #[test]
    fn compact_uses_baseline_today_for_pre_window_race_day_tsb() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"today":"2026-09-20","ctl":31.61,"atl":38.42,"tsb":-6.81},"days":[{"date":"2026-09-21","ctl":30.86,"atl":32.93,"tsb":-2.07,"source":"input","planned_tss":45,"tss_source":"planned"},{"date":"2026-09-22","ctl":30.5,"atl":30.0,"tsb":-1.06,"source":"input","rest_day":true,"planned_tss":0,"tss_source":"planned"},{"date":"2026-09-26","ctl":30.38,"atl":27.03,"tsb":-6.12,"source":"input","planned_tss":80,"tss_source":"planned"}]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("baseline(2026-09-20) ctl=31.61 atl=38.42 tsb=-6.81"));
        assert!(load.contains(
            "race_day_tsb=-6.81@2026-09-20 (race_day_tsb_source=baseline_pre_window, race load not included)"
        ));
        assert!(!load.contains("race_tss="));
        assert!(load.contains("days=["));
        assert!(load.contains("2026-09-21:-2.07@tss=45:src=planned"));
        assert!(load.contains("2026-09-22:-1.06(rest)@tss=0:src=planned"));
        assert!(load.contains("tsb_min=-6.12@2026-09-26"));
    }

    #[test]
    fn compact_uses_baseline_applied_load_for_race_day_tsb() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"today":"2026-09-20","ctl":32.1,"atl":39.0,"tsb":-6.9},"baseline_applied_load":{"date":"2026-09-20","tss":107,"tss_source":"estimated"},"days":[{"date":"2026-09-21","ctl":31.0,"atl":35.0,"tsb":-4.0,"source":"input","planned_tss":40,"tss_source":"planned"}]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains(
            "race_day_tsb=-6.9@2026-09-20 (race_day_tsb_source=baseline_with_estimated_race_load, race_tss=107)"
        ));
        assert!(!load.contains("baseline_pre_window"));
        assert!(!load.contains("race load not included"));
    }

    #[test]
    fn compact_does_not_invent_baseline_pre_window_when_today_is_in_days() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"today":"2026-02-01","ctl":10,"atl":5,"tsb":5},"days":[{"date":"2026-02-01","ctl":11,"atl":6,"tsb":5,"source":"input"}]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("baseline(2026-02-01)"));
        assert!(load.contains("race_day_tsb=n/a"));
        assert!(!load.contains("baseline_pre_window"));
    }

    #[test]
    fn compact_surfaces_race_tss_for_in_window_estimated_race() {
        let (assistant, tool) = tool_pair(
            "1",
            "simulate_forward_load",
            r#"{"baseline":{"today":"2026-05-11","ctl":50,"atl":40,"tsb":10},"days":[{"date":"2026-05-12","ctl":52,"atl":48,"tsb":-2,"source":"future_event","tss_source":"estimated","planned_tss":128}],"notes":["2026-05-12: race TSS estimated (tss=128.00, duration_s=7200); not measured"]}"#,
        );
        let evidence = evidence_from_transcript(&[assistant, tool]);
        let load = evidence.load.as_deref().unwrap();
        assert!(load.contains("race_day_tsb=-2@2026-05-12 (race_day_tsb_source=estimated)"));
        assert!(load.contains("race_tss=128 (source=estimated)"));
        assert!(load.contains("2026-05-12:-2@tss=128:src=estimated"));
    }

    #[test]
    fn compact_trims_middle_day_pairs_only() {
        let pairs: Vec<String> = (0..12)
            .map(|i| format!("2026-05-{:02}:{i}", 10 + i))
            .collect();
        let mut protected = std::collections::BTreeSet::new();
        protected.insert(0);
        protected.insert(11);
        protected.insert(5);
        protected.insert(9);
        let rendered = super::trim_forward_load_day_pairs(pairs, &protected, 80);
        assert!(rendered.contains("2026-05-10:0"));
        assert!(rendered.contains("2026-05-21:11"));
        assert!(rendered.contains("2026-05-15:5"));
        assert!(rendered.contains("2026-05-19:9"));
        assert!(rendered.contains("…[trimmed]"));
        assert!(!rendered.contains("2026-05-14:4"));
    }

    #[test]
    fn compact_insufficient_data_power_and_w_prime_are_citable() {
        let (a1, t1) = tool_pair(
            "1",
            "selected_workout_power_curve",
            r#"{"status":"insufficient_data","reason":"no watts power stream in workout details","date":"2026-05-05","workout_id":"cw-1"}"#,
        );
        let (a2, t2) = tool_pair(
            "2",
            "get_w_prime_balance",
            r#"{"status":"insufficient_data","reason":"no valid power samples available","date":"2026-05-05","workout_id":"cw-1"}"#,
        );
        let evidence = evidence_from_transcript(&[a1, t1, a2, t2]);
        assert!(evidence
            .power_curve
            .as_ref()
            .unwrap()
            .contains("status=insufficient_data"));
        assert!(evidence
            .power_curve
            .as_ref()
            .unwrap()
            .contains("no watts power stream"));
        assert!(evidence
            .w_prime
            .as_ref()
            .unwrap()
            .contains("status=insufficient_data"));
        assert!(evidence.load.is_none());
    }
}
