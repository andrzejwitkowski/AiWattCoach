use crate::domain::identity::Clock;

use super::{
    hash_text, LlmChatMessage, LlmChatRequest, LlmContextCache, LlmContextCacheRepository,
    LlmError, LlmProvider,
};

pub struct LlmChatRequestInput {
    pub user_id: String,
    pub system_prompt: String,
    pub stable_context: String,
    pub volatile_context: String,
    pub conversation: Vec<LlmChatMessage>,
    pub cache_scope_key: Option<String>,
    pub cache_key: Option<String>,
    pub reusable_cache_id: Option<String>,
}

pub struct ReusableContextCacheLookup<'a> {
    pub repository: Option<&'a dyn LlmContextCacheRepository>,
    pub user_id: &'a str,
    pub provider: &'a LlmProvider,
    pub model: &'a str,
    pub scope_key: Option<&'a str>,
    pub context_hash: &'a str,
    pub now_epoch_seconds: i64,
}

pub struct ReusableContextCacheUpsert<'a> {
    pub repository: Option<&'a dyn LlmContextCacheRepository>,
    pub user_id: &'a str,
    pub provider: &'a LlmProvider,
    pub model: &'a str,
    pub scope_key: Option<&'a str>,
    pub context_hash: &'a str,
    pub provider_cache_id: Option<&'a str>,
    pub expires_at_epoch_seconds: Option<i64>,
    pub now_epoch_seconds: i64,
}

pub fn current_date_string<Time>(clock: &Time) -> String
where
    Time: Clock,
{
    chrono::DateTime::from_timestamp(clock.now_epoch_seconds(), 0)
        .map(|time| time.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| {
            chrono::DateTime::UNIX_EPOCH
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()
        })
}

pub fn current_datetime_rfc3339<Time>(clock: &Time) -> String
where
    Time: Clock,
{
    epoch_seconds_to_rfc3339(clock.now_epoch_seconds())
}

pub fn epoch_seconds_to_rfc3339(epoch_seconds: i64) -> String {
    chrono::DateTime::from_timestamp(epoch_seconds, 0)
        .map(|time| time.to_rfc3339())
        .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH.to_rfc3339())
}

pub fn conversation_timing_volatile_context(
    current_conversation_epoch_seconds: i64,
    latest_user_message_epoch_seconds: Option<i64>,
) -> String {
    let mut context = format!(
        "conversation_timing={{\"currentConversationDatetime\":\"{}\",\"instruction\":\"Treat this timing as authoritative for now in this conversation. Do not assume the athlete is writing the day after a workout unless timestamps explicitly show that.\"}}",
        epoch_seconds_to_rfc3339(current_conversation_epoch_seconds)
    );

    if let Some(latest_user_message_epoch_seconds) = latest_user_message_epoch_seconds {
        context.push_str(&format!(
            "\nlatest_user_message_datetime={}",
            epoch_seconds_to_rfc3339(latest_user_message_epoch_seconds)
        ));
    }

    context
}

pub fn reusable_context_cache_key(system_prompt: &str, stable_context: &str) -> String {
    hash_text(&format!("{system_prompt}\n{stable_context}"))
}

pub fn build_chat_request(input: LlmChatRequestInput) -> LlmChatRequest {
    LlmChatRequest {
        user_id: input.user_id,
        system_prompt: input.system_prompt,
        stable_context: input.stable_context,
        volatile_context: input.volatile_context,
        conversation: input.conversation,
        cache_scope_key: input.cache_scope_key,
        cache_key: input.cache_key,
        reusable_cache_id: input.reusable_cache_id,
        ..Default::default()
    }
}

pub async fn find_reusable_context_cache(
    input: ReusableContextCacheLookup<'_>,
) -> Result<Option<LlmContextCache>, LlmError> {
    if input.provider != &LlmProvider::Gemini {
        return Ok(None);
    }

    let Some(repository) = input.repository else {
        return Ok(None);
    };
    let Some(scope_key) = input.scope_key else {
        return Ok(None);
    };

    repository
        .find_reusable(
            input.user_id,
            input.provider,
            input.model,
            scope_key,
            input.context_hash,
            input.now_epoch_seconds,
        )
        .await
}

pub async fn persist_reusable_context_cache(
    input: ReusableContextCacheUpsert<'_>,
) -> Result<Option<LlmContextCache>, LlmError> {
    if input.provider != &LlmProvider::Gemini {
        return Ok(None);
    }

    let Some(repository) = input.repository else {
        return Ok(None);
    };
    let Some(scope_key) = input.scope_key else {
        return Ok(None);
    };
    let Some(provider_cache_id) = input.provider_cache_id else {
        return Ok(None);
    };

    repository
        .upsert(LlmContextCache {
            user_id: input.user_id.to_string(),
            provider: input.provider.clone(),
            model: input.model.to_string(),
            scope_key: scope_key.to_string(),
            context_hash: input.context_hash.to_string(),
            provider_cache_id: provider_cache_id.to_string(),
            expires_at_epoch_seconds: input.expires_at_epoch_seconds,
            created_at_epoch_seconds: input.now_epoch_seconds,
            updated_at_epoch_seconds: input.now_epoch_seconds,
        })
        .await
        .map(Some)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures::executor::block_on;

    use super::{
        build_chat_request, conversation_timing_volatile_context, current_date_string,
        current_datetime_rfc3339, epoch_seconds_to_rfc3339, find_reusable_context_cache,
        persist_reusable_context_cache, reusable_context_cache_key, LlmChatRequestInput,
        ReusableContextCacheLookup, ReusableContextCacheUpsert,
    };
    use crate::domain::{
        identity::Clock,
        llm::{
            BoxFuture, LlmChatMessage, LlmContextCache, LlmContextCacheRepository, LlmProvider,
            LlmToolChoice,
        },
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
    struct RecordingCacheRepository {
        last_upsert: Arc<Mutex<Option<LlmContextCache>>>,
    }

    impl LlmContextCacheRepository for RecordingCacheRepository {
        fn find_reusable(
            &self,
            user_id: &str,
            provider: &LlmProvider,
            model: &str,
            scope_key: &str,
            context_hash: &str,
            now_epoch_seconds: i64,
        ) -> BoxFuture<Result<Option<LlmContextCache>, crate::domain::llm::LlmError>> {
            let cache = LlmContextCache {
                user_id: user_id.to_string(),
                provider: provider.clone(),
                model: model.to_string(),
                scope_key: scope_key.to_string(),
                context_hash: context_hash.to_string(),
                provider_cache_id: "cache-1".to_string(),
                expires_at_epoch_seconds: Some(now_epoch_seconds + 10),
                created_at_epoch_seconds: now_epoch_seconds,
                updated_at_epoch_seconds: now_epoch_seconds,
            };
            Box::pin(async move { Ok(Some(cache)) })
        }

        fn upsert(
            &self,
            cache: LlmContextCache,
        ) -> BoxFuture<Result<LlmContextCache, crate::domain::llm::LlmError>> {
            let last_upsert = self.last_upsert.clone();
            Box::pin(async move {
                *last_upsert
                    .lock()
                    .expect("cache lock should not be poisoned") = Some(cache.clone());
                Ok(cache)
            })
        }

        fn delete_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<(), crate::domain::llm::LlmError>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn current_date_string_formats_clock_date() {
        let date = current_date_string(&FixedClock {
            now_epoch_seconds: 1_746_489_600,
        });

        assert_eq!(date, "2025-05-06");
    }

    #[test]
    fn current_date_string_falls_back_for_invalid_timestamp() {
        let date = current_date_string(&FixedClock {
            now_epoch_seconds: i64::MAX,
        });

        assert_eq!(date, "1970-01-01");
    }

    #[test]
    fn current_datetime_rfc3339_formats_clock_time() {
        let datetime = current_datetime_rfc3339(&FixedClock {
            now_epoch_seconds: 1_746_489_600,
        });

        assert_eq!(datetime, "2025-05-06T00:00:00+00:00");
    }

    #[test]
    fn epoch_seconds_to_rfc3339_falls_back_for_invalid_timestamp() {
        assert_eq!(
            epoch_seconds_to_rfc3339(i64::MAX),
            "1970-01-01T00:00:00+00:00"
        );
    }

    #[test]
    fn conversation_timing_volatile_context_includes_current_and_latest_user_time() {
        let context = conversation_timing_volatile_context(1_746_489_600, Some(1_746_490_200));

        assert!(context.contains("currentConversationDatetime"));
        assert!(context.contains("2025-05-06T00:00:00+00:00"));
        assert!(context.contains("latest_user_message_datetime=2025-05-06T00:10:00+00:00"));
        assert!(context.contains("Do not assume the athlete is writing the day after a workout"));
    }

    #[test]
    fn build_chat_request_keeps_tool_fields_at_default() {
        let request = build_chat_request(LlmChatRequestInput {
            user_id: "user-1".to_string(),
            system_prompt: "system".to_string(),
            stable_context: "stable".to_string(),
            volatile_context: "volatile".to_string(),
            conversation: vec![LlmChatMessage::user("hello")],
            cache_scope_key: Some("scope".to_string()),
            cache_key: Some("cache-key".to_string()),
            reusable_cache_id: Some("provider-cache".to_string()),
        });

        assert!(request.tools.is_empty());
        assert_eq!(request.tool_choice, LlmToolChoice::None);
    }

    #[test]
    fn reusable_context_cache_key_is_stable() {
        let first = reusable_context_cache_key("system", "stable");
        let second = reusable_context_cache_key("system", "stable");

        assert_eq!(first, second);
    }

    #[test]
    fn find_reusable_context_cache_ignores_non_gemini_provider() {
        let repository = RecordingCacheRepository::default();

        let cache = block_on(find_reusable_context_cache(ReusableContextCacheLookup {
            repository: Some(&repository),
            user_id: "user-1",
            provider: &LlmProvider::OpenAi,
            model: "gpt-4o-mini",
            scope_key: Some("scope"),
            context_hash: "hash",
            now_epoch_seconds: 10,
        }))
        .expect("lookup should succeed");

        assert_eq!(cache, None);
    }

    #[test]
    fn persist_reusable_context_cache_upserts_gemini_cache() {
        let repository = RecordingCacheRepository::default();

        let cache = block_on(persist_reusable_context_cache(ReusableContextCacheUpsert {
            repository: Some(&repository),
            user_id: "user-1",
            provider: &LlmProvider::Gemini,
            model: "gemini-2.5-flash",
            scope_key: Some("scope"),
            context_hash: "hash",
            provider_cache_id: Some("provider-cache"),
            expires_at_epoch_seconds: Some(50),
            now_epoch_seconds: 40,
        }))
        .expect("upsert should succeed")
        .expect("cache should be persisted");

        assert_eq!(cache.provider_cache_id, "provider-cache");
        assert_eq!(cache.context_hash, "hash");
        assert_eq!(
            repository
                .last_upsert
                .lock()
                .expect("cache lock should not be poisoned")
                .as_ref()
                .expect("cache should be recorded")
                .scope_key,
            "scope"
        );
    }
}
