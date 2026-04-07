use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryState, AthleteSummaryUseCases,
        EnsuredAthleteSummary,
    },
    intervals::{
        Activity, ActivityDetails, ActivityMetrics, ActivityStream, DateRange, Event,
        IntervalsError, IntervalsUseCases,
    },
    llm::{BoxFuture as LlmBoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError},
    settings::{
        AiAgentsConfig, AnalysisOptions, BoxFuture as SettingsBoxFuture, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsRepository,
    },
    workout_summary::{
        BoxFuture as WorkoutBoxFuture, CoachReplyClaimResult, CoachReplyOperation,
        CoachReplyOperationRepository, CoachReplyOperationStatus, ConversationMessage,
        WorkoutRecap, WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
    },
};
use chrono::Utc;

type SummaryKey = (String, String);
type ReplyOperationKey = (String, String, String);

mod athlete_summary;
mod cache;
mod intervals;
mod settings;
mod workout_summary;

pub(crate) use athlete_summary::InMemoryAthleteSummaryService;
pub(crate) use cache::InMemoryLlmContextCacheRepository;
pub(crate) use intervals::{sample_activity, InMemoryIntervalsService};
pub(crate) use settings::{ai_config, sample_user_settings, InMemoryUserSettingsRepository};
pub(crate) use workout_summary::{
    sample_summary, InMemoryCoachReplyOperationRepository, InMemoryWorkoutSummaryRepository,
};
