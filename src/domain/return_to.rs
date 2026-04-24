pub fn sanitize_return_to(raw_return_to: Option<String>) -> Option<String> {
    raw_return_to.and_then(|value| {
        let trimmed = value.trim();
        let lower = trimmed.to_ascii_lowercase();
        let path_end = trimmed
            .find(|character| ['?', '#'].contains(&character))
            .unwrap_or(trimmed.len());

        if trimmed.is_empty()
            || !trimmed.starts_with('/')
            || trimmed.starts_with("//")
            || trimmed[..path_end].contains(':')
            || trimmed.contains('\\')
            || trimmed.chars().any(|character| character.is_control())
            || lower.contains("%0d")
            || lower.contains("%0a")
        {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::sanitize_return_to;

    #[test]
    fn sanitize_return_to_rejects_absolute_urls() {
        assert_eq!(
            sanitize_return_to(Some("https://evil.example/settings".to_string())),
            None
        );
    }

    #[test]
    fn sanitize_return_to_keeps_timestamp_query_parameters() {
        assert_eq!(
            sanitize_return_to(Some(
                "/settings?since=2024-01-01T10:00:00Z#time=10:30".to_string(),
            )),
            Some("/settings?since=2024-01-01T10:00:00Z#time=10:30".to_string())
        );
    }

    #[test]
    fn sanitize_return_to_rejects_colons_in_the_path() {
        assert_eq!(sanitize_return_to(Some("/settings:evil".to_string())), None);
    }
}
