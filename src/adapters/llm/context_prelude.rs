pub(crate) fn non_empty_context_parts<'a>(
    parts: [(&'static str, &'a str); 3],
) -> Vec<(&'static str, &'a str)> {
    parts
        .into_iter()
        .filter(|(_, content)| !content.trim().is_empty())
        .collect()
}
