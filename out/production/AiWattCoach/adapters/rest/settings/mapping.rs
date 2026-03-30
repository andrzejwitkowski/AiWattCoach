use crate::domain::settings::{
    mask_sensitive, validation, AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings,
};

use super::dto::{
    AiAgentsDto, CyclingDto, IntervalsDto, OptionsDto, UpdateAiAgentsRequest, UpdateCyclingRequest,
    UpdateIntervalsRequest, UpdateOptionsRequest, UserSettingsDto,
};

pub(super) fn map_settings_to_dto(settings: &UserSettings) -> UserSettingsDto {
    UserSettingsDto {
        ai_agents: AiAgentsDto {
            openai_api_key: mask_sensitive(&settings.ai_agents.openai_api_key),
            openai_api_key_set: settings.ai_agents.openai_api_key.is_some(),
            gemini_api_key: mask_sensitive(&settings.ai_agents.gemini_api_key),
            gemini_api_key_set: settings.ai_agents.gemini_api_key.is_some(),
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
) -> AiAgentsConfig {
    AiAgentsConfig {
        openai_api_key: body
            .openai_api_key
            .or(current.ai_agents.openai_api_key.clone()),
        gemini_api_key: body
            .gemini_api_key
            .or(current.ai_agents.gemini_api_key.clone()),
    }
}

pub(super) fn map_intervals_update(
    body: UpdateIntervalsRequest,
    current: &UserSettings,
) -> IntervalsConfig {
    IntervalsConfig {
        api_key: body.api_key.or(current.intervals.api_key.clone()),
        athlete_id: body.athlete_id.or(current.intervals.athlete_id.clone()),
        connected: current.intervals.connected,
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
