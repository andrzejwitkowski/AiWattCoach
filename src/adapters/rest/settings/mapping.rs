use crate::domain::settings::{
    mask_sensitive, validation, AiAgentsConfig, AnalysisOptions, AvailabilityDay,
    AvailabilitySettings, CyclingSettings, IntervalsConfig, SettingsError, UserSettings, Weekday,
};

use super::dto::{
    AiAgentsDto, AvailabilityDayDto, AvailabilityDto, CyclingDto, IntervalsDto, OptionsDto,
    UpdateAiAgentsRequest, UpdateAvailabilityRequest, UpdateCyclingRequest, UpdateIntervalsRequest,
    UpdateOptionsRequest, UserSettingsDto,
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
        availability: AvailabilityDto {
            configured: settings.availability.configured,
            days: settings
                .availability
                .days
                .iter()
                .map(|day| AvailabilityDayDto {
                    weekday: day.weekday.as_str().to_string(),
                    available: day.available,
                    max_duration_minutes: day.max_duration_minutes,
                })
                .collect(),
        },
        cycling: CyclingDto {
            full_name: settings.cycling.full_name.clone(),
            age: settings.cycling.age,
            height_cm: settings.cycling.height_cm,
            weight_kg: settings.cycling.weight_kg,
            ftp_watts: settings.cycling.ftp_watts,
            hr_max_bpm: settings.cycling.hr_max_bpm,
            vo2_max: settings.cycling.vo2_max,
            athlete_prompt: settings.cycling.athlete_prompt.clone(),
            medications: settings.cycling.medications.clone(),
            athlete_notes: settings.cycling.athlete_notes.clone(),
            last_zone_update_epoch_seconds: settings.cycling.last_zone_update_epoch_seconds,
        },
    }
}

pub(super) fn map_availability_update(
    body: UpdateAvailabilityRequest,
    _current: &UserSettings,
) -> Result<AvailabilitySettings, SettingsError> {
    let days: Vec<AvailabilityDay> = body
        .days
        .into_iter()
        .map(|day| {
            let weekday = day.weekday.trim().to_lowercase();
            let weekday = Weekday::parse(&weekday).ok_or_else(|| {
                SettingsError::Validation(format!(
                    "availability weekday '{}' is invalid",
                    day.weekday
                ))
            })?;

            Ok(AvailabilityDay {
                weekday,
                available: day.available,
                max_duration_minutes: day.max_duration_minutes,
            })
        })
        .collect::<Result<_, _>>()?;
    let configured = days.iter().any(|day| day.available);

    validation::validate_availability(AvailabilitySettings { configured, days })
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
    let api_key = apply_field_update(
        normalize_string_input(body.api_key),
        current.intervals.api_key.clone(),
    );
    let athlete_id = apply_field_update(
        normalize_string_input(body.athlete_id),
        current.intervals.athlete_id.clone(),
    );

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
    let full_name_update = normalize_string_input(body.full_name);
    let athlete_prompt_update = normalize_string_input(body.athlete_prompt);
    let medications_update = normalize_string_input(body.medications);
    let athlete_notes_update = normalize_string_input(body.athlete_notes);
    let athlete_prompt = validation::validate_optional_profile_text(
        "athletePrompt",
        apply_field_update(
            athlete_prompt_update,
            current.cycling.athlete_prompt.clone(),
        ),
        6000,
    )?;
    let medications = validation::validate_optional_profile_text(
        "medications",
        apply_field_update(medications_update, current.cycling.medications.clone()),
        4000,
    )?;
    let athlete_notes = validation::validate_optional_profile_text(
        "athleteNotes",
        apply_field_update(athlete_notes_update, current.cycling.athlete_notes.clone()),
        8000,
    )?;

    Ok(CyclingSettings {
        full_name: apply_field_update(full_name_update, current.cycling.full_name.clone()),
        age,
        height_cm,
        weight_kg,
        ftp_watts,
        hr_max_bpm,
        vo2_max,
        athlete_prompt,
        medications,
        athlete_notes,
        last_zone_update_epoch_seconds: current.cycling.last_zone_update_epoch_seconds,
    })
}
