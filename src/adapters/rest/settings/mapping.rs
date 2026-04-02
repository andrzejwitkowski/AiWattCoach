use crate::domain::settings::{
    mask_sensitive, validation, AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings,
};

use super::dto::{
    AiAgentsDto, CyclingDto, IntervalsDto, OptionsDto, UpdateAiAgentsRequest, UpdateCyclingRequest,
    UpdateIntervalsRequest, UpdateOptionsRequest, UserSettingsDto,
};
use super::input::{
    apply_field_update, normalize_string_input, parse_provider_settings_input, FieldUpdate,
};

pub(super) fn map_settings_to_dto(settings: &UserSettings) -> UserSettingsDto {
    UserSettingsDto {
        ai_agents: AiAgentsDto {
            openai_api_key: mask_sensitive(&settings.ai_agents.openai_api_key),
            openai_api_key_set: settings.ai_agents.openai_api_key.is_some(),
            gemini_api_key: mask_sensitive(&settings.ai_agents.gemini_api_key),
            gemini_api_key_set: settings.ai_agents.gemini_api_key.is_some(),
            openrouter_api_key: mask_sensitive(&settings.ai_agents.openrouter_api_key),
            openrouter_api_key_set: settings.ai_agents.openrouter_api_key.is_some(),
            selected_provider: settings
                .ai_agents
                .selected_provider
                .as_ref()
                .map(|provider| provider.as_str().to_string()),
            selected_model: settings.ai_agents.selected_model.clone(),
        },
        intervals: IntervalsDto {
            api_key: mask_sensitive(&settings.intervals.api_key),
            api_key_set: settings.intervals.api_key.is_some(),
            athlete_id: settings.intervals.athlete_id.clone(),
            connected: settings.intervals.connected,
        },
        options: OptionsDto {
            analyze_without_heart_rate: settings.options.analyze_without_heart_rate,
        },
        cycling: CyclingDto {
            full_name: settings.cycling.full_name.clone(),
            age: settings.cycling.age,
            height_cm: settings.cycling.height_cm,
            weight_kg: settings.cycling.weight_kg,
            ftp_watts: settings.cycling.ftp_watts,
            hr_max_bpm: settings.cycling.hr_max_bpm,
            vo2_max: settings.cycling.vo2_max,
            last_zone_update_epoch_seconds: settings.cycling.last_zone_update_epoch_seconds,
        },
    }
}

pub(super) fn map_ai_agents_update(
    body: UpdateAiAgentsRequest,
    current: &UserSettings,
) -> Result<AiAgentsConfig, SettingsError> {
    let selected_provider_update = parse_provider_settings_input(body.selected_provider)?;
    let selected_model_update = normalize_string_input(body.selected_model);
    let openai_api_key = normalize_string_input(body.openai_api_key);
    let gemini_api_key = normalize_string_input(body.gemini_api_key);
    let openrouter_api_key = normalize_string_input(body.openrouter_api_key);

    let provider_changed = match &selected_provider_update {
        FieldUpdate::Missing => false,
        FieldUpdate::Clear => current.ai_agents.selected_provider.is_some(),
        FieldUpdate::Set(provider) => {
            current.ai_agents.selected_provider.as_ref() != Some(provider)
        }
    };

    let selected_provider = apply_field_update(
        selected_provider_update,
        current.ai_agents.selected_provider.clone(),
    );
    let selected_model = validation::validate_ai_model(if provider_changed {
        apply_field_update(selected_model_update, None)
    } else {
        apply_field_update(
            selected_model_update,
            current.ai_agents.selected_model.clone(),
        )
    })?;

    match (&selected_provider, &selected_model) {
        (Some(_), None) => {
            return Err(SettingsError::Validation(
                "selectedModel must not be empty".to_string(),
            ))
        }
        (None, Some(_)) => {
            return Err(SettingsError::Validation(
                "selectedProvider must not be empty".to_string(),
            ))
        }
        _ => {}
    }

    Ok(AiAgentsConfig {
        openai_api_key: apply_field_update(
            openai_api_key,
            current.ai_agents.openai_api_key.clone(),
        ),
        gemini_api_key: apply_field_update(
            gemini_api_key,
            current.ai_agents.gemini_api_key.clone(),
        ),
        openrouter_api_key: apply_field_update(
            openrouter_api_key,
            current.ai_agents.openrouter_api_key.clone(),
        ),
        selected_provider: validation::validate_ai_provider(selected_provider)?,
        selected_model,
    })
}

pub(super) fn map_intervals_update(
    body: UpdateIntervalsRequest,
    current: &UserSettings,
) -> IntervalsConfig {
    let api_key =
        normalize_optional_patch_value(body.api_key).or(current.intervals.api_key.clone());
    let athlete_id =
        normalize_optional_patch_value(body.athlete_id).or(current.intervals.athlete_id.clone());

    IntervalsConfig {
        connected: if api_key != current.intervals.api_key
            || athlete_id != current.intervals.athlete_id
        {
            false
        } else {
            current.intervals.connected
        },
        api_key,
        athlete_id,
    }
}

pub(super) fn map_options_update(
    body: UpdateOptionsRequest,
    current: &UserSettings,
) -> AnalysisOptions {
    AnalysisOptions {
        analyze_without_heart_rate: body
            .analyze_without_heart_rate
            .unwrap_or(current.options.analyze_without_heart_rate),
    }
}

fn normalize_optional_patch_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn map_cycling_update(
    body: UpdateCyclingRequest,
    current: &UserSettings,
) -> Result<CyclingSettings, SettingsError> {
    let age = validation::validate_cycling_age(body.age.or(current.cycling.age))?;
    let height_cm =
        validation::validate_cycling_height(body.height_cm.or(current.cycling.height_cm))?;
    let weight_kg =
        validation::validate_cycling_weight(body.weight_kg.or(current.cycling.weight_kg))?;
    let ftp_watts = validation::validate_cycling_ftp(body.ftp_watts.or(current.cycling.ftp_watts))?;
    let hr_max_bpm =
        validation::validate_cycling_hr(body.hr_max_bpm.or(current.cycling.hr_max_bpm))?;
    let vo2_max = validation::validate_cycling_vo2(body.vo2_max.or(current.cycling.vo2_max))?;

    Ok(CyclingSettings {
        full_name: body.full_name.or(current.cycling.full_name.clone()),
        age,
        height_cm,
        weight_kg,
        ftp_watts,
        hr_max_bpm,
        vo2_max,
        last_zone_update_epoch_seconds: current.cycling.last_zone_update_epoch_seconds,
    })
}
