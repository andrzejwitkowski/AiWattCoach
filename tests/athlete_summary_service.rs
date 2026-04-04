use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryGenerator, AthleteSummaryRepository,
        AthleteSummaryService, AthleteSummaryUseCases,
    },
    identity::Clock,
    llm::{LlmCacheUsage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage},
};

#[derive(Clone)]
struct FixedClock {
    now_epoch_seconds: i64,
}

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.now_epoch_seconds
    }
}

#[derive(Clone, Default)]
struct InMemoryAthleteSummaryRepository {
    summary: Arc<Mutex<Option<AthleteSummary>>>,
    find_calls: Arc<Mutex<u32>>,
}

impl InMemoryAthleteSummaryRepository {
    fn with_summary(summary: AthleteSummary) -> Self {
        Self {
            summary: Arc::new(Mutex::new(Some(summary))),
            find_calls: Arc::new(Mutex::new(0)),
        }
    }
}

impl AthleteSummaryRepository for InMemoryAthleteSummaryRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummary>, AthleteSummaryError>,
    > {
        *self.find_calls.lock().unwrap() += 1;
        let summary = self.summary.lock().unwrap().clone();
        Box::pin(async move { Ok(summary) })
    }

    fn upsert(
        &self,
        summary: AthleteSummary,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        let store = self.summary.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(summary.clone());
            Ok(summary)
        })
    }
}

impl InMemoryAthleteSummaryRepository {
    fn find_call_count(&self) -> u32 {
        *self.find_calls.lock().unwrap()
    }
}

#[derive(Clone)]
struct StubGenerator {
    calls: Arc<Mutex<u32>>,
    message: String,
}

impl StubGenerator {
    fn new(message: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(0)),
            message: message.to_string(),
        }
    }

    fn call_count(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

impl AthleteSummaryGenerator for StubGenerator {
    fn generate(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<LlmChatResponse, LlmError>> {
        let calls = self.calls.clone();
        let message = self.message.clone();
        Box::pin(async move {
            *calls.lock().unwrap() += 1;
            Ok(LlmChatResponse {
                provider: LlmProvider::OpenRouter,
                model: "google/gemini-3-flash-preview".to_string(),
                message,
                provider_request_id: None,
                usage: LlmTokenUsage::default(),
                cache: LlmCacheUsage::default(),
            })
        })
    }
}

#[tokio::test]
async fn ensure_fresh_summary_generates_when_missing() {
    let repository = InMemoryAthleteSummaryRepository::default();
    let generator = StubGenerator::new("fresh summary");
    let service = AthleteSummaryService::new(
        repository,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: 1_775_564_800,
        },
    );

    let summary = service.ensure_fresh_summary("user-1").await.unwrap();

    assert_eq!(summary.summary_text, "fresh summary");
    assert_eq!(generator.call_count(), 1);
}

#[tokio::test]
async fn ensure_fresh_summary_reuses_summary_generated_this_week() {
    let repository = InMemoryAthleteSummaryRepository::with_summary(AthleteSummary {
        user_id: "user-1".to_string(),
        summary_text: "existing".to_string(),
        generated_at_epoch_seconds: 1_775_520_000,
        created_at_epoch_seconds: 1_775_520_000,
        updated_at_epoch_seconds: 1_775_520_000,
        provider: Some("openrouter".to_string()),
        model: Some("google/gemini-3-flash-preview".to_string()),
    });
    let generator = StubGenerator::new("fresh summary");
    let service = AthleteSummaryService::new(
        repository,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: 1_775_564_800,
        },
    );

    let summary = service.ensure_fresh_summary("user-1").await.unwrap();

    assert_eq!(summary.summary_text, "existing");
    assert_eq!(generator.call_count(), 0);
}

#[tokio::test]
async fn ensure_fresh_summary_regenerates_when_older_than_monday() {
    let repository = InMemoryAthleteSummaryRepository::with_summary(AthleteSummary {
        user_id: "user-1".to_string(),
        summary_text: "old".to_string(),
        generated_at_epoch_seconds: 1_775_347_200,
        created_at_epoch_seconds: 1_775_347_200,
        updated_at_epoch_seconds: 1_775_347_200,
        provider: Some("openrouter".to_string()),
        model: Some("google/gemini-3-flash-preview".to_string()),
    });
    let generator = StubGenerator::new("fresh summary");
    let service = AthleteSummaryService::new(
        repository,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: 1_775_564_800,
        },
    );

    let summary = service.ensure_fresh_summary("user-1").await.unwrap();

    assert_eq!(summary.summary_text, "fresh summary");
    assert_eq!(generator.call_count(), 1);
}

#[tokio::test]
async fn generate_summary_force_true_regenerates_even_when_fresh() {
    let repository = InMemoryAthleteSummaryRepository::with_summary(AthleteSummary {
        user_id: "user-1".to_string(),
        summary_text: "existing".to_string(),
        generated_at_epoch_seconds: 1_775_520_000,
        created_at_epoch_seconds: 1_775_520_000,
        updated_at_epoch_seconds: 1_775_520_000,
        provider: Some("openrouter".to_string()),
        model: Some("google/gemini-3-flash-preview".to_string()),
    });
    let generator = StubGenerator::new("forced summary");
    let service = AthleteSummaryService::new(
        repository,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: 1_775_564_800,
        },
    );

    let summary = service.generate_summary("user-1", true).await.unwrap();

    assert_eq!(summary.summary_text, "forced summary");
    assert_eq!(generator.call_count(), 1);
}

#[tokio::test]
async fn ensure_fresh_summary_reads_repository_once_when_summary_is_fresh() {
    let repository = InMemoryAthleteSummaryRepository::with_summary(AthleteSummary {
        user_id: "user-1".to_string(),
        summary_text: "existing".to_string(),
        generated_at_epoch_seconds: 1_775_520_000,
        created_at_epoch_seconds: 1_775_520_000,
        updated_at_epoch_seconds: 1_775_520_000,
        provider: Some("openrouter".to_string()),
        model: Some("google/gemini-3-flash-preview".to_string()),
    });
    let generator = StubGenerator::new("fresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: 1_775_564_800,
        },
    );

    let summary = service.ensure_fresh_summary("user-1").await.unwrap();

    assert_eq!(summary.summary_text, "existing");
    assert_eq!(generator.call_count(), 0);
    assert_eq!(repository.find_call_count(), 1);
}
