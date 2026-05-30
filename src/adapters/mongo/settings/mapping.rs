use crate::adapters::mongo::time::{
    optional_epoch_seconds_to_bson_datetime, resolve_optional_epoch_seconds,
    resolve_required_epoch_seconds,
};
use crate::domain::llm::LlmProvider;
use crate::domain::settings::{
    validation, AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings,
    CyclingSettings, IntervalsConfig, SettingsError, UserSettings, WahooConfig, Weekday,
};

use super::documents::{
    AiAgentsDocument, AvailabilityDayDocument, AvailabilityDocument, CyclingDocument,
    IntervalsDocument, OptionsDocument, SettingsDocument, WahooDocument,
};

pub(super) fn map_document_to_domain(doc: SettingsDocument) -> Result<UserSettings, SettingsError> {
    Ok(UserSettings {
        user_id: doc.user_id,
        ai_agents: AiAgentsConfig {
            openai_api_key: doc.ai_agents.openai_api_key,
            gemini_api_key: doc.ai_agents.gemini_api_key,
            openrouter_api_key: doc.ai_agents.openrouter_api_key,
            deepseek_api_key: doc.ai_agents.deepseek_api_key,
            selected_provider: doc
                .ai_agents
                .selected_provider
                .as_deref()
                .and_then(LlmProvider::parse),
            selected_model: doc.ai_agents.selected_model,
        },
        intervals: IntervalsConfig {
            api_key: doc.intervals.api_key,
            athlete_id: doc.intervals.athlete_id,
            connected: doc.intervals.connected,
        },
        options: AnalysisOptions {
            analyze_without_heart_rate: doc.options.analyze_without_heart_rate,
        },
        wahoo: WahooConfig {
            access_token: doc.wahoo.access_token,
            refresh_token: doc.wahoo.refresh_token,
            expires_at_epoch_seconds: resolve_optional_epoch_seconds(
                doc.wahoo.expires_at,
                doc.wahoo.expires_at_epoch_seconds,
            ),
            user_id: doc.wahoo.user_id,
            connected: doc.wahoo.connected,
            updated_at_epoch_seconds: resolve_optional_epoch_seconds(
                doc.wahoo.updated_at,
                doc.wahoo.updated_at_epoch_seconds,
            ),
        },
        availability: map_document_availability_to_domain(doc.availability),
        cycling: map_document_cycling_to_domain(doc.cycling),
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            doc.created_at,
            doc.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(SettingsError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            doc.updated_at,
            doc.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(SettingsError::Repository)?,
    })
}

pub(super) fn map_domain_to_document(settings: &UserSettings) -> SettingsDocument {
    SettingsDocument {
        id: None,
        user_id: settings.user_id.clone(),
        ai_agents: AiAgentsDocument {
            openai_api_key: settings.ai_agents.openai_api_key.clone(),
            gemini_api_key: settings.ai_agents.gemini_api_key.clone(),
            openrouter_api_key: settings.ai_agents.openrouter_api_key.clone(),
            deepseek_api_key: settings.ai_agents.deepseek_api_key.clone(),
            selected_provider: settings
                .ai_agents
                .selected_provider
                .as_ref()
                .map(|provider| provider.as_str().to_string()),
            selected_model: settings.ai_agents.selected_model.clone(),
        },
        intervals: IntervalsDocument {
            api_key: settings.intervals.api_key.clone(),
            athlete_id: settings.intervals.athlete_id.clone(),
            connected: settings.intervals.connected,
            updated_at_epoch_seconds: None,
            updated_at: None,
        },
        wahoo: WahooDocument {
            access_token: settings.wahoo.access_token.clone(),
            refresh_token: settings.wahoo.refresh_token.clone(),
            expires_at_epoch_seconds: settings.wahoo.expires_at_epoch_seconds,
            expires_at: optional_epoch_seconds_to_bson_datetime(
                settings.wahoo.expires_at_epoch_seconds,
                "wahoo.expires_at",
            )
            .expect("wahoo.expires_at should fit BSON DateTime"),
            user_id: settings.wahoo.user_id,
            connected: settings.wahoo.connected,
            updated_at_epoch_seconds: settings.wahoo.updated_at_epoch_seconds,
            updated_at: optional_epoch_seconds_to_bson_datetime(
                settings.wahoo.updated_at_epoch_seconds,
                "wahoo.updated_at",
            )
            .expect("wahoo.updated_at should fit BSON DateTime"),
        },
        options: OptionsDocument {
            analyze_without_heart_rate: settings.options.analyze_without_heart_rate,
        },
        availability: map_domain_availability_to_document(&settings.availability),
        cycling: map_domain_cycling_to_document(&settings.cycling),
        created_at_epoch_seconds: Some(settings.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(settings.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: Some(settings.updated_at_epoch_seconds),
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(settings.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
    }
}

fn map_document_cycling_to_domain(cycling: CyclingDocument) -> CyclingSettings {
    CyclingSettings {
        full_name: cycling.full_name,
        age: cycling.age,
        height_cm: cycling.height_cm,
        weight_kg: cycling.weight_kg,
        ftp_watts: cycling.ftp_watts,
        hr_max_bpm: cycling.hr_max_bpm,
        vo2_max: cycling.vo2_max,
        athlete_prompt: cycling.athlete_prompt,
        medications: cycling.medications,
        athlete_notes: cycling.athlete_notes,
        last_zone_update_epoch_seconds: resolve_optional_epoch_seconds(
            cycling.last_zone_update_at,
            cycling.last_zone_update_epoch_seconds,
        ),
    }
}

pub(super) fn map_domain_cycling_to_document(cycling: &CyclingSettings) -> CyclingDocument {
    CyclingDocument {
        full_name: cycling.full_name.clone(),
        age: cycling.age,
        height_cm: cycling.height_cm,
        weight_kg: cycling.weight_kg,
        ftp_watts: cycling.ftp_watts,
        hr_max_bpm: cycling.hr_max_bpm,
        vo2_max: cycling.vo2_max,
        athlete_prompt: cycling.athlete_prompt.clone(),
        medications: cycling.medications.clone(),
        athlete_notes: cycling.athlete_notes.clone(),
        last_zone_update_epoch_seconds: cycling.last_zone_update_epoch_seconds,
        last_zone_update_at: optional_epoch_seconds_to_bson_datetime(
            cycling.last_zone_update_epoch_seconds,
            "cycling.last_zone_update_at",
        )
        .expect("cycling.last_zone_update_at should fit BSON DateTime"),
    }
}

pub(super) fn map_document_availability_to_domain(
    document: AvailabilityDocument,
) -> AvailabilitySettings {
    let has_complete_explicit_week = has_complete_explicit_week(&document.days);
    let repaired_days = repair_availability_days(document.days);

    match validation::validate_availability(AvailabilitySettings {
        configured: document.configured && has_complete_explicit_week,
        days: repaired_days,
    }) {
        Ok(mut availability) => {
            if !has_complete_explicit_week {
                availability.configured = false;
            }
            availability
        }
        Err(error) => {
            tracing::warn!(error = %error, "falling back to default availability after unrecoverable settings document");
            AvailabilitySettings::default()
        }
    }
}

fn repair_availability_days(days: Vec<AvailabilityDayDocument>) -> Vec<AvailabilityDay> {
    use std::collections::BTreeMap;

    let mut repaired = BTreeMap::<Weekday, AvailabilityDay>::new();

    for day in days {
        let weekday = day.weekday.trim().to_lowercase();
        let Some(weekday) = Weekday::parse(&weekday) else {
            continue;
        };

        repaired.insert(
            weekday,
            AvailabilityDay {
                weekday,
                available: day.available
                    && day
                        .max_duration_minutes
                        .is_some_and(validation::is_allowed_availability_duration),
                max_duration_minutes: if day.available
                    && day
                        .max_duration_minutes
                        .is_some_and(validation::is_allowed_availability_duration)
                {
                    day.max_duration_minutes
                } else {
                    None
                },
            },
        );
    }

    Weekday::ALL
        .into_iter()
        .map(|weekday| {
            repaired.remove(&weekday).unwrap_or(AvailabilityDay {
                weekday,
                available: false,
                max_duration_minutes: None,
            })
        })
        .collect()
}

fn has_complete_explicit_week(days: &[AvailabilityDayDocument]) -> bool {
    let normalized_weekdays = days
        .iter()
        .map(|day| day.weekday.trim().to_lowercase())
        .collect::<Vec<_>>();
    let distinct_valid_weekdays = days
        .iter()
        .map(|day| day.weekday.trim().to_lowercase())
        .filter_map(|weekday| Weekday::parse(&weekday))
        .collect::<std::collections::BTreeSet<_>>();

    distinct_valid_weekdays.len() == 7 && normalized_weekdays.len() == 7
}

pub(super) fn map_domain_availability_to_document(
    availability: &AvailabilitySettings,
) -> AvailabilityDocument {
    AvailabilityDocument {
        configured: availability.configured,
        days: availability
            .days
            .iter()
            .map(|day| AvailabilityDayDocument {
                weekday: day.weekday.as_str().to_string(),
                available: day.available,
                max_duration_minutes: day.max_duration_minutes,
            })
            .collect(),
    }
}
