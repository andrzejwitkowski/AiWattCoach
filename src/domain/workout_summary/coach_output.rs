use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use super::CoachQuestion;

const MAX_COACH_QUESTIONS: usize = 6;
const MIN_QUESTION_ANSWERS: usize = 2;
const MAX_QUESTION_ANSWERS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCoachReply {
    pub content: String,
    pub questions: Vec<CoachQuestion>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct CoachReplyEnvelope {
    summary: String,
    #[serde(default)]
    #[schemars(length(max = 6))]
    questions: Vec<CoachQuestionEnvelope>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct CoachQuestionEnvelope {
    question: String,
    #[schemars(length(min = 2, max = 6), inner(length(min = 1)))]
    answers: Vec<String>,
    #[serde(rename = "freeTextLabel")]
    #[schemars(rename = "freeTextLabel")]
    free_text_label: Option<String>,
}

pub fn parse_coach_reply(raw: &str) -> Result<ParsedCoachReply, String> {
    let payload = extract_json_payload(raw);
    if !looks_like_json(payload) {
        if looks_like_malformed_coach_reply_json(payload) {
            return Err(
                "assistant reply looks like malformed JSON for workout summary coach schema"
                    .to_string(),
            );
        }
        return parse_plain_text_reply(payload);
    }

    let parsed: CoachReplyEnvelope = serde_json::from_str(payload)
        .map_err(|error| format!("assistant reply is not valid JSON: {error}"))?;

    let content = trim_required_text(parsed.summary, "assistant reply summary")?;
    let questions = normalize_questions(parsed.questions)?;

    Ok(ParsedCoachReply { content, questions })
}

pub fn workout_summary_coach_reply_json_schema() -> String {
    serde_json::to_string_pretty(&schema_for!(CoachReplyEnvelope))
        .expect("workout summary coach reply schema should serialize")
}

fn parse_plain_text_reply(payload: &str) -> Result<ParsedCoachReply, String> {
    let content = trim_required_text(payload.to_string(), "assistant reply")?;
    Ok(ParsedCoachReply {
        content,
        questions: Vec::new(),
    })
}

fn looks_like_json(payload: &str) -> bool {
    payload.starts_with('{') || payload.starts_with('[')
}

fn looks_like_malformed_coach_reply_json(payload: &str) -> bool {
    let lower = payload.to_ascii_lowercase();
    let mentions_schema_keys = lower.contains("summary") || lower.contains("questions");
    mentions_schema_keys && (payload.contains('{') || payload.contains('['))
}

fn extract_json_payload(raw: &str) -> &str {
    extract_fenced_json(raw.trim()).unwrap_or_else(|| raw.trim())
}

fn extract_fenced_json(raw: &str) -> Option<&str> {
    let mut cursor = 0;
    let mut last = None;
    while let Some(rel) = raw[cursor..].find("```") {
        let open = cursor + rel + 3;
        let suffix = &raw[open..];
        let body = suffix
            .strip_prefix("json")
            .or_else(|| suffix.strip_prefix("JSON"))
            .unwrap_or(suffix);
        let body_start = open + suffix.len().saturating_sub(body.len());
        let inner = body.trim_start();
        let inner_start = body_start + body.len().saturating_sub(inner.len());
        let Some(close) = inner.find("```") else {
            cursor = open + 1;
            continue;
        };
        if let Some(payload) = json_block_payload(inner[..close].trim()) {
            last = Some(payload);
        }
        cursor = inner_start + close + 3;
    }
    last
}

fn json_block_payload(block: &str) -> Option<&str> {
    if block.starts_with('{') || block.starts_with('[') {
        return Some(block);
    }
    block
        .split_once('\n')
        .map(|(_, rest)| rest.trim())
        .filter(|rest| rest.starts_with('{') || rest.starts_with('['))
}

fn normalize_questions(
    raw_questions: Vec<CoachQuestionEnvelope>,
) -> Result<Vec<CoachQuestion>, String> {
    if raw_questions.len() > MAX_COACH_QUESTIONS {
        return Err(format!(
            "assistant reply must contain at most {MAX_COACH_QUESTIONS} questions"
        ));
    }

    raw_questions
        .into_iter()
        .enumerate()
        .map(|(index, question)| normalize_question(index, question))
        .collect()
}

fn normalize_question(
    index: usize,
    question: CoachQuestionEnvelope,
) -> Result<CoachQuestion, String> {
    let question_text = trim_required_text(question.question, "assistant question")?;
    let answers = normalize_answers(question.answers)?;
    let free_text_label = question
        .free_text_label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(CoachQuestion {
        id: format!("question-{}", index + 1),
        question: question_text,
        answers,
        free_text_label,
    })
}

fn normalize_answers(answers: Vec<String>) -> Result<Vec<String>, String> {
    if !(MIN_QUESTION_ANSWERS..=MAX_QUESTION_ANSWERS).contains(&answers.len()) {
        return Err(format!(
            "assistant question must contain between {MIN_QUESTION_ANSWERS} and {MAX_QUESTION_ANSWERS} answers"
        ));
    }

    answers
        .into_iter()
        .map(|answer| trim_required_text(answer, "assistant question answer"))
        .collect()
}

fn trim_required_text(value: String, field_name: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} must not be empty"));
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_coach_reply, workout_summary_coach_reply_json_schema};
    use serde_json::Value;

    #[test]
    fn parse_coach_reply_returns_summary_and_questions() {
        let parsed = parse_coach_reply(
            r#"{
                "summary": "Legs were the main limiter. Give me a bit more detail before we adjust the next block.",
                "questions": [
                    {
                        "question": "What limited you most?",
                        "answers": ["Legs", "Breathing", "Fueling", "Pain"],
                        "freeTextLabel": "Add context"
                    },
                    {
                        "question": "How ready are you for the next 2-3 days?",
                        "answers": ["Ready for quality", "Easy only", "Need recovery"]
                    }
                ]
            }"#,
        )
        .expect("coach reply should parse");

        assert_eq!(parsed.content, "Legs were the main limiter. Give me a bit more detail before we adjust the next block.");
        assert_eq!(parsed.questions.len(), 2);
        assert_eq!(parsed.questions[0].id, "question-1");
        assert_eq!(parsed.questions[0].answers[0], "Legs");
        assert_eq!(
            parsed.questions[0].free_text_label.as_deref(),
            Some("Add context")
        );
        assert_eq!(parsed.questions[1].id, "question-2");
        assert!(parsed.questions[1].free_text_label.is_none());
    }

    #[test]
    fn parse_coach_reply_accepts_empty_questions_when_coach_is_ready() {
        let parsed = parse_coach_reply(
            r#"{
                "summary": "I have everything I need to generate the next plan. Save this summary when you're ready.",
                "questions": []
            }"#,
        )
        .expect("coach reply without questions should parse");

        assert!(parsed.questions.is_empty());
    }

    #[test]
    fn parse_coach_reply_defaults_missing_questions_to_empty() {
        let parsed = parse_coach_reply(
            r#"{
                "summary": "I have everything I need to generate the next plan. Save this summary when you're ready."
            }"#,
        )
        .expect("coach reply without questions field should parse");

        assert!(parsed.questions.is_empty());
    }

    #[test]
    fn parse_coach_reply_accepts_json_code_fence() {
        let parsed = parse_coach_reply(
            "```json\n{\n  \"summary\": \"Good session.\",\n  \"questions\": []\n}\n```",
        )
        .expect("code fenced JSON should parse");

        assert_eq!(parsed.content, "Good session.");
    }

    #[test]
    fn parse_coach_reply_accepts_json_code_fence_after_prose() {
        let parsed = parse_coach_reply(
            "Teraz mam pełny obraz sesji.\n\n```json\n{\n  \"summary\": \"Solid ride.\",\n  \"questions\": []\n}\n```",
        )
        .expect("prose before fenced JSON should parse");

        assert_eq!(parsed.content, "Solid ride.");
    }

    #[test]
    fn parse_coach_reply_uses_last_valid_json_fence() {
        let parsed = parse_coach_reply(
            "Draft\n```json\n{\"summary\":\"First\",\"questions\":[]}\n```\nFinal\n```json\n{\"summary\":\"Second\",\"questions\":[]}\n```",
        )
        .expect("last fenced JSON should win");

        assert_eq!(parsed.content, "Second");
    }

    #[test]
    fn parse_coach_reply_rejects_malformed_json() {
        let error = parse_coach_reply(r#"{"summary":"Missing close""#).unwrap_err();

        assert!(error.contains("assistant reply is not valid JSON"));
    }

    #[test]
    fn parse_coach_reply_accepts_plain_text_as_summary_without_questions() {
        let parsed = parse_coach_reply("This was a useful endurance ride. Save the summary.")
            .expect("plain text coach reply should remain usable");

        assert_eq!(
            parsed.content,
            "This was a useful endurance ride. Save the summary."
        );
        assert!(parsed.questions.is_empty());
    }

    #[test]
    fn parse_coach_reply_rejects_json_like_text_near_schema() {
        let error = parse_coach_reply(
            r#"Here is the JSON: {"summary":"Useful ride","questions":[{"question":"Limiter?""#,
        )
        .unwrap_err();

        assert!(error.contains("assistant reply looks like malformed JSON"));
    }

    #[test]
    fn workout_summary_coach_reply_schema_matches_parser_envelope_shape() {
        let schema: Value = serde_json::from_str(&workout_summary_coach_reply_json_schema())
            .expect("schema should be valid JSON");

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["summary"]["type"], "string");
        assert_eq!(schema["properties"]["questions"]["type"], "array");
        assert_eq!(schema["properties"]["questions"]["maxItems"], 6);
    }

    #[test]
    fn parse_coach_reply_rejects_empty_summary() {
        let error = parse_coach_reply(
            r#"{
                "summary": "   ",
                "questions": []
            }"#,
        )
        .unwrap_err();

        assert_eq!(error, "assistant reply summary must not be empty");
    }

    #[test]
    fn parse_coach_reply_accepts_plain_code_fence() {
        let parsed = parse_coach_reply(
            "```\n{\n  \"summary\": \"Solid ride.\",\n  \"questions\": []\n}\n```",
        )
        .expect("plain code fenced JSON should parse");

        assert_eq!(parsed.content, "Solid ride.");
    }

    #[test]
    fn parse_coach_reply_accepts_uppercase_json_code_fence() {
        let parsed = parse_coach_reply(
            "```JSON\n{\n  \"summary\": \"Solid ride.\",\n  \"questions\": []\n}\n```",
        )
        .expect("uppercase code fenced JSON should parse");

        assert_eq!(parsed.content, "Solid ride.");
    }

    #[test]
    fn parse_coach_reply_rejects_question_with_too_few_answers() {
        let error = parse_coach_reply(
            r#"{
                "summary": "Need one more data point.",
                "questions": [
                    {
                        "question": "What limited you?",
                        "answers": ["Legs"]
                    }
                ]
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error,
            "assistant question must contain between 2 and 6 answers"
        );
    }

    #[test]
    fn parse_coach_reply_rejects_question_with_too_many_answers() {
        let error = parse_coach_reply(
            r#"{
                "summary": "Too many choices.",
                "questions": [
                    {
                        "question": "What limited you?",
                        "answers": ["A", "B", "C", "D", "E", "F", "G"]
                    }
                ]
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error,
            "assistant question must contain between 2 and 6 answers"
        );
    }

    #[test]
    fn parse_coach_reply_rejects_more_than_6_questions() {
        let question_block = r#"{"question":"Q","answers":["A","B"]}"#;
        let questions = std::iter::repeat_n(question_block, 7)
            .collect::<Vec<_>>()
            .join(",");
        let input = format!(r#"{{"summary":"Overloaded.","questions":[{questions}]}}"#);

        let error = parse_coach_reply(&input).unwrap_err();

        assert_eq!(error, "assistant reply must contain at most 6 questions");
    }
}
