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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoachReplyEnvelope {
    summary: String,
    #[serde(default)]
    questions: Vec<CoachQuestionEnvelope>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoachQuestionEnvelope {
    question: String,
    answers: Vec<String>,
    #[serde(rename = "freeTextLabel")]
    free_text_label: Option<String>,
}

pub fn parse_coach_reply(raw: &str) -> Result<ParsedCoachReply, String> {
    let payload = extract_json_payload(raw);
    let parsed: CoachReplyEnvelope = serde_json::from_str(payload)
        .map_err(|error| format!("assistant reply is not valid JSON: {error}"))?;

    let content = trim_required_text(parsed.summary, "assistant reply summary")?;
    let questions = normalize_questions(parsed.questions)?;

    Ok(ParsedCoachReply { content, questions })
}

fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();

    if let Some(stripped) = trimmed.strip_prefix("```") {
        let inner = stripped.trim().trim_end_matches("```").trim();

        if inner.starts_with('{') || inner.starts_with('[') {
            return inner;
        }

        if let Some((_, rest)) = inner.split_once('\n') {
            return rest.trim();
        }

        return inner;
    }

    trimmed
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
    use super::parse_coach_reply;

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
    fn parse_coach_reply_rejects_invalid_json() {
        let error = parse_coach_reply("not json").unwrap_err();

        assert!(error.contains("assistant reply is not valid JSON"));
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
