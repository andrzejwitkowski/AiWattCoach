use crate::domain::{llm::LlmProvider, settings::SettingsError};

use super::dto::OptionalStringInput;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FieldUpdate<T> {
    Missing,
    Clear,
    Set(T),
}

pub(super) fn apply_field_update<T>(update: FieldUpdate<T>, current: Option<T>) -> Option<T> {
    match update {
        FieldUpdate::Missing => current,
        FieldUpdate::Clear => None,
        FieldUpdate::Set(value) => Some(value),
    }
}

pub(super) fn normalize_string_input(input: OptionalStringInput) -> FieldUpdate<String> {
    match input {
        OptionalStringInput::Missing => FieldUpdate::Missing,
        OptionalStringInput::Null => FieldUpdate::Clear,
        OptionalStringInput::Value(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                FieldUpdate::Clear
            } else {
                FieldUpdate::Set(trimmed.to_string())
            }
        }
    }
}

pub(super) fn parse_provider_input<E, F>(
    input: OptionalStringInput,
    invalid_error: F,
) -> Result<FieldUpdate<LlmProvider>, E>
where
    F: FnOnce() -> E,
{
    match normalize_string_input(input) {
        FieldUpdate::Missing => Ok(FieldUpdate::Missing),
        FieldUpdate::Clear => Ok(FieldUpdate::Clear),
        FieldUpdate::Set(value) => LlmProvider::parse(&value)
            .map(FieldUpdate::Set)
            .ok_or_else(invalid_error),
    }
}

pub(super) fn parse_provider_settings_input(
    input: OptionalStringInput,
) -> Result<FieldUpdate<LlmProvider>, SettingsError> {
    parse_provider_input(input, || {
        SettingsError::Validation(
            "selectedProvider must be one of: openai, gemini, openrouter".to_string(),
        )
    })
}

pub(super) fn used_saved_value<T>(update: &FieldUpdate<T>, current: &Option<T>) -> bool {
    matches!(update, FieldUpdate::Missing) && current.is_some()
}
