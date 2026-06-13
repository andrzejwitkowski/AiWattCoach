pub(crate) fn non_empty_context_parts<'a>(
    parts: [(&'static str, &'a str); 3],
) -> Vec<(&'static str, &'a str)> {
    parts
        .into_iter()
        .filter(|(_, content)| !content.trim().is_empty())
        .collect()
}

pub(crate) use crate::domain::llm::packed_training_context_legend_with_guidance;
