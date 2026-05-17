pub fn coach_reply_json(summary: impl AsRef<str>) -> String {
    format!(
        r#"{{"summary":{},"questions":[]}}"#,
        serde_json::to_string(summary.as_ref()).expect("summary should serialize")
    )
}

pub fn coach_reply_json_with_question(summary: impl AsRef<str>) -> String {
    format!(
        r#"{{"summary":{},"questions":[{{"question":"What limited you most today?","answers":["Legs","Breathing","Fueling","Pain","Other"],"freeTextLabel":"Add details if useful"}}]}}"#,
        serde_json::to_string(summary.as_ref()).expect("summary should serialize")
    )
}
