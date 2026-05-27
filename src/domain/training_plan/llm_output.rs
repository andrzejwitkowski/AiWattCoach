use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

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
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct TrainingPlanLlmEnvelopePayload {
    plan: String,
    description: Option<String>,
}

pub fn parse_training_plan_llm_envelope(
    payload: &str,
) -> Result<TrainingPlanLlmEnvelope, TrainingPlanError> {
    let parsed: TrainingPlanLlmEnvelopePayload =
        serde_json::from_str(payload).map_err(|error| {
            TrainingPlanError::Unavailable(format!("invalid training plan llm json: {error}"))
        })?;

    if parsed.plan.trim().is_empty() {
        return Err(TrainingPlanError::Unavailable(
            "training plan llm json missing non-empty plan".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{parse_training_plan_llm_envelope, training_plan_llm_envelope_json_schema};
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
    fn parse_training_plan_llm_envelope_rejects_unknown_fields() {
        let error = parse_training_plan_llm_envelope(
            r#"{
                "plan": "Mon: Rest",
                "unexpected": true
            }"#,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            TrainingPlanError::Unavailable(message)
                if message.starts_with("invalid training plan llm json:")
                    && message.contains("unknown field `unexpected`")
        ));
    }

    #[test]
    fn training_plan_llm_envelope_schema_matches_contract() {
        let schema: Value = serde_json::from_str(&training_plan_llm_envelope_json_schema())
            .expect("schema should be valid JSON");

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
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
}
