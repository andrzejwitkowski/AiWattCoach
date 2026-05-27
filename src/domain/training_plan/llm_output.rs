use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::error::Category as JsonErrorCategory;

use super::TrainingPlanError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrainingPlanLlmEnvelope {
    plan: String,
    description: Option<String>,
}

impl TrainingPlanLlmEnvelope {
    pub fn plan(&self) -> &str {
        &self.plan
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[derive(Deserialize, JsonSchema)]
struct TrainingPlanLlmEnvelopePayload {
    plan: String,
    description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrainingPlanLlmEnvelopeParseErrorKind {
    RecoverableFormatting,
    InvalidEnvelope,
}

pub fn parse_training_plan_llm_envelope(
    payload: &str,
) -> Result<TrainingPlanLlmEnvelope, TrainingPlanError> {
    parse_training_plan_llm_envelope_detailed(payload).map_err(|error| error.0)
}

pub(crate) fn should_retry_training_plan_llm_envelope_repair(payload: &str) -> bool {
    matches!(
        parse_training_plan_llm_envelope_detailed(payload),
        Err((
            _,
            TrainingPlanLlmEnvelopeParseErrorKind::RecoverableFormatting
        ))
    )
}

fn parse_training_plan_llm_envelope_detailed(
    payload: &str,
) -> Result<TrainingPlanLlmEnvelope, (TrainingPlanError, TrainingPlanLlmEnvelopeParseErrorKind)> {
    let payload = extract_json_payload(payload);
    let parsed: TrainingPlanLlmEnvelopePayload =
        serde_json::from_str(payload).map_err(|error| {
            (
                TrainingPlanError::Unavailable(format!("invalid training plan llm json: {error}")),
                classify_training_plan_llm_json_error(&error),
            )
        })?;

    if parsed.plan.trim().is_empty() {
        return Err((
            TrainingPlanError::Unavailable(
                "training plan llm json missing non-empty plan".to_string(),
            ),
            TrainingPlanLlmEnvelopeParseErrorKind::InvalidEnvelope,
        ));
    }

    Ok(TrainingPlanLlmEnvelope {
        plan: parsed.plan,
        description: parsed.description,
    })
}

pub fn training_plan_llm_envelope_json_schema() -> String {
    serde_json::to_string_pretty(&schema_for!(TrainingPlanLlmEnvelopePayload))
        .expect("training plan llm envelope schema should serialize")
}

fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();

    if let Some(stripped) = trimmed.strip_prefix("```") {
        let inner = stripped.trim().trim_end_matches("```").trim();
        let candidate = if inner.starts_with('{') || inner.starts_with('[') {
            inner
        } else if let Some((_, rest)) = inner.split_once('\n') {
            rest.trim()
        } else {
            inner
        };

        return extract_balanced_json_payload(candidate).unwrap_or(candidate);
    }

    extract_balanced_json_payload(trimmed).unwrap_or(trimmed)
}

fn extract_balanced_json_payload(raw: &str) -> Option<&str> {
    let start = raw
        .char_indices()
        .find(|(_, character)| matches!(character, '{' | '['))?
        .0;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in raw[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match character {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }

            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                let expected = stack.pop()?;
                if character != expected {
                    return None;
                }

                if stack.is_empty() {
                    let end = start + index + character.len_utf8();
                    return Some(&raw[start..end]);
                }
            }
            _ => {}
        }
    }

    None
}

fn classify_training_plan_llm_json_error(
    error: &serde_json::Error,
) -> TrainingPlanLlmEnvelopeParseErrorKind {
    match error.classify() {
        JsonErrorCategory::Syntax | JsonErrorCategory::Eof => {
            TrainingPlanLlmEnvelopeParseErrorKind::RecoverableFormatting
        }
        JsonErrorCategory::Data | JsonErrorCategory::Io => {
            TrainingPlanLlmEnvelopeParseErrorKind::InvalidEnvelope
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_training_plan_llm_envelope, should_retry_training_plan_llm_envelope_repair,
        training_plan_llm_envelope_json_schema,
    };
    use crate::domain::training_plan::TrainingPlanError;
    use serde_json::{json, Value};

    #[test]
    fn parse_training_plan_llm_envelope_parses_valid_payload_with_optional_description() {
        let parsed = parse_training_plan_llm_envelope(
            r#"{
                "plan": "Mon: Rest\nTue: 45m endurance",
                "description": "Recover into the next quality block."
            }"#,
        )
        .expect("valid envelope should parse");

        assert_eq!(parsed.plan(), "Mon: Rest\nTue: 45m endurance");
        assert_eq!(
            parsed.description(),
            Some("Recover into the next quality block.")
        );
    }

    #[test]
    fn parse_training_plan_llm_envelope_accepts_json_code_fence() {
        let parsed = parse_training_plan_llm_envelope(
            "```json\n{\n  \"plan\": \"2026-05-28\\nRest Day: recovery\",\n  \"description\": \"Easy day.\"\n}\n```",
        )
        .expect("code-fenced JSON envelope should parse");

        assert_eq!(parsed.plan(), "2026-05-28\nRest Day: recovery");
        assert_eq!(parsed.description(), Some("Easy day."));
    }

    #[test]
    fn parse_training_plan_llm_envelope_accepts_plain_code_fence() {
        let parsed = parse_training_plan_llm_envelope(
            "```\n{\n  \"plan\": \"2026-05-28\\nRest Day: recovery\"\n}\n```",
        )
        .expect("plain code-fenced JSON envelope should parse");

        assert_eq!(parsed.plan(), "2026-05-28\nRest Day: recovery");
        assert_eq!(parsed.description(), None);
    }

    #[test]
    fn parse_training_plan_llm_envelope_ignores_extra_top_level_metadata() {
        let parsed = parse_training_plan_llm_envelope(
            r#"{
                "plan": "2026-05-28\nRest Day: recovery",
                "description": "Easy day.",
                "simulated_load": {
                    "ctl_start": 48.49,
                    "ctl_end": 50.45,
                    "tsb_min": -10.03
                }
            }"#,
        )
        .expect("extra top-level LLM metadata should not block usable plans");

        assert_eq!(parsed.plan(), "2026-05-28\nRest Day: recovery");
        assert_eq!(parsed.description(), Some("Easy day."));
    }

    #[test]
    fn parse_training_plan_llm_envelope_extracts_json_object_from_surrounding_text() {
        let parsed = parse_training_plan_llm_envelope(
            "Here is the corrected envelope:\n{\n  \"plan\": \"2026-05-28\\nRest Day: recovery\",\n  \"description\": \"Easy day.\"\n}\nPlease use it.",
        )
        .expect("embedded JSON envelope should parse");

        assert_eq!(parsed.plan(), "2026-05-28\nRest Day: recovery");
        assert_eq!(parsed.description(), Some("Easy day."));
    }

    #[test]
    fn parse_training_plan_llm_envelope_ignores_unknown_fields() {
        let parsed = parse_training_plan_llm_envelope(
            r#"{
                "plan": "Mon: Rest",
                "unexpected": true
            }"#,
        )
        .expect("unknown top-level metadata should be ignored");

        assert_eq!(parsed.plan(), "Mon: Rest");
        assert_eq!(parsed.description(), None);
    }

    #[test]
    fn training_plan_llm_envelope_schema_matches_contract() {
        let schema: Value = serde_json::from_str(&training_plan_llm_envelope_json_schema())
            .expect("schema should be valid JSON");

        assert_eq!(schema["type"], "object");
        assert!(schema.get("additionalProperties").is_none());
        assert_eq!(schema["properties"]["plan"]["type"], "string");
        assert_eq!(
            schema["properties"]["description"],
            json!({ "type": ["string", "null"] })
        );
        assert_eq!(schema["required"], json!(["plan"]));
    }

    #[test]
    fn parse_training_plan_llm_envelope_rejects_empty_plan() {
        let error = parse_training_plan_llm_envelope(
            r#"{
                "plan": "   "
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error,
            TrainingPlanError::Unavailable(
                "training plan llm json missing non-empty plan".to_string()
            )
        );
    }

    #[test]
    fn parse_training_plan_llm_envelope_rejects_malformed_json_after_extraction() {
        let error = parse_training_plan_llm_envelope(
            "Here is the envelope:\n{\n  \"plan\": \"2026-05-28\\nRest Day: recovery\",\n",
        )
        .unwrap_err();

        assert!(matches!(
            error,
            TrainingPlanError::Unavailable(message)
                if message.starts_with("invalid training plan llm json:")
        ));
    }

    #[test]
    fn should_retry_training_plan_llm_envelope_repair_for_syntax_failure() {
        assert!(should_retry_training_plan_llm_envelope_repair(
            "Here is the envelope:\n{\n  \"plan\": \"2026-05-28\\nRest Day: recovery\",\n"
        ));
    }

    #[test]
    fn should_not_retry_training_plan_llm_envelope_repair_for_missing_plan() {
        assert!(!should_retry_training_plan_llm_envelope_repair(
            r#"{
                "description": "missing required plan"
            }"#
        ));
    }

    #[test]
    fn should_not_retry_training_plan_llm_envelope_repair_for_wrong_plan_type() {
        assert!(!should_retry_training_plan_llm_envelope_repair(
            r#"{
                "plan": 123
            }"#
        ));
    }
}
