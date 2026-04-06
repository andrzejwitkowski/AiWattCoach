use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryState, AthleteSummaryUseCases,
        EnsuredAthleteSummary,
    },
    identity::{Clock, IdGenerator},
    llm::{LlmCacheUsage, LlmChatResponse, LlmProvider, LlmTokenUsage},
    training_plan::{GeneratedTrainingPlan, TrainingPlanError, TrainingPlanUseCases},
    workout_summary::{
        BoxFuture, CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationRepository,
        CoachReplyOperationStatus, ConversationMessage, MessageRole, WorkoutCoach, WorkoutRecap,
        WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryService,
    },
};

type ReplyOperationKey = (String, String, String);
type ReplyOperationStore = BTreeMap<ReplyOperationKey, CoachReplyOperation>;

mod identity;
mod reply_operations;
mod services;
mod summary_repository;

pub(crate) use identity::{TestClock, TestIdGenerator};
pub(crate) use reply_operations::InMemoryCoachReplyOperationRepository;
pub(crate) use services::{
    default_dev_coach, existing_summary, test_service, test_service_with_coach,
    test_service_with_coach_and_athlete_summary, test_service_with_training_plan,
    RecordingTrainingPlanService, StubAthleteSummaryService,
};
pub(crate) use summary_repository::InMemoryWorkoutSummaryRepository;
