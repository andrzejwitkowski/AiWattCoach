use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use mongodb::Client;

use crate::domain::athlete_summary::AthleteSummaryUseCases;
use crate::domain::calendar::CalendarUseCases;
use crate::domain::calendar_labels::CalendarLabelsUseCases;
use crate::domain::completed_workouts::{
    CompletedWorkoutAdminUseCases, CompletedWorkoutReadUseCases,
};
use crate::domain::identity::IdentityUseCases;
use crate::domain::intervals::{IntervalsConnectionTester, IntervalsUseCases};
use crate::domain::llm::{LlmChatPort, UserLlmConfigProvider};
use crate::domain::races::RaceUseCases;
use crate::domain::settings::UserSettingsUseCases;
use crate::domain::workout_summary::WorkoutSummaryUseCases;

#[derive(Clone)]
pub struct AppState {
    pub app_name: String,
    pub mongo_database: String,
    pub mongo_client: Client,
    pub client_log_ingestion_enabled: bool,
    pub identity_service: Option<Arc<dyn IdentityUseCases>>,
    pub calendar_service: Option<Arc<dyn CalendarUseCases>>,
    pub calendar_labels_service: Option<Arc<dyn CalendarLabelsUseCases>>,
    pub completed_workout_service: Option<Arc<dyn CompletedWorkoutReadUseCases>>,
    pub completed_workout_admin_service: Option<Arc<dyn CompletedWorkoutAdminUseCases>>,
    pub intervals_service: Option<Arc<dyn IntervalsUseCases>>,
    pub race_service: Option<Arc<dyn RaceUseCases>>,
    pub settings_service: Option<Arc<dyn UserSettingsUseCases>>,
    pub athlete_summary_service: Option<Arc<dyn AthleteSummaryUseCases>>,
    pub workout_summary_service: Option<Arc<dyn WorkoutSummaryUseCases>>,
    pub llm_chat_service: Option<Arc<dyn LlmChatPort>>,
    pub llm_config_provider: Option<Arc<dyn UserLlmConfigProvider>>,
    pub intervals_connection_tester: Option<Arc<dyn IntervalsConnectionTester>>,
    pub whitelist_rate_limiter: WhitelistRateLimiter,
    pub session_cookie_name: String,
    pub session_cookie_same_site: String,
    pub secure_session_cookie: bool,
    pub session_ttl_hours: u64,
}

#[derive(Clone)]
pub struct WhitelistRateLimiter {
    max_attempts: usize,
    window: Duration,
    attempts_by_ip: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl Default for WhitelistRateLimiter {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(60))
    }
}

impl WhitelistRateLimiter {
    pub fn new(max_attempts: usize, window: Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            window,
            attempts_by_ip: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn check(&self, client_ip: &str) -> bool {
        let now = Instant::now();
        let mut attempts_by_ip = self
            .attempts_by_ip
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        attempts_by_ip.retain(|_, attempts| {
            attempts.retain(|attempt| now.duration_since(*attempt) < self.window);
            !attempts.is_empty()
        });
        let attempts = attempts_by_ip.entry(client_ip.to_string()).or_default();

        if attempts.len() >= self.max_attempts {
            return false;
        }

        attempts.push(now);
        true
    }
}

impl AppState {
    pub fn new(
        app_name: impl Into<String>,
        mongo_database: impl Into<String>,
        mongo_client: Client,
    ) -> Self {
        Self {
            app_name: app_name.into(),
            mongo_database: mongo_database.into(),
            mongo_client,
            client_log_ingestion_enabled: false,
            identity_service: None,
            calendar_service: None,
            calendar_labels_service: None,
            completed_workout_service: None,
            completed_workout_admin_service: None,
            intervals_service: None,
            race_service: None,
            settings_service: None,
            athlete_summary_service: None,
            workout_summary_service: None,
            llm_chat_service: None,
            llm_config_provider: None,
            intervals_connection_tester: None,
            whitelist_rate_limiter: WhitelistRateLimiter::default(),
            session_cookie_name: "aiwattcoach_session".to_string(),
            session_cookie_same_site: "lax".to_string(),
            secure_session_cookie: false,
            session_ttl_hours: 24,
        }
    }

    pub fn with_identity_service(
        mut self,
        identity_service: Arc<dyn IdentityUseCases>,
        session_cookie_name: impl Into<String>,
        session_cookie_same_site: impl Into<String>,
        secure_session_cookie: bool,
        session_ttl_hours: u64,
    ) -> Self {
        self.identity_service = Some(identity_service);
        self.session_cookie_name = session_cookie_name.into();
        self.session_cookie_same_site = session_cookie_same_site.into();
        self.secure_session_cookie = secure_session_cookie;
        self.session_ttl_hours = session_ttl_hours;
        self
    }

    pub fn with_settings_service(
        mut self,
        settings_service: Arc<dyn UserSettingsUseCases>,
    ) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    pub fn with_calendar_service(mut self, calendar_service: Arc<dyn CalendarUseCases>) -> Self {
        self.calendar_service = Some(calendar_service);
        self
    }

    pub fn with_calendar_labels_service(
        mut self,
        calendar_labels_service: Arc<dyn CalendarLabelsUseCases>,
    ) -> Self {
        self.calendar_labels_service = Some(calendar_labels_service);
        self
    }

    pub fn with_completed_workout_service(
        mut self,
        completed_workout_service: Arc<dyn CompletedWorkoutReadUseCases>,
    ) -> Self {
        self.completed_workout_service = Some(completed_workout_service);
        self
    }

    pub fn with_completed_workout_admin_service(
        mut self,
        completed_workout_admin_service: Arc<dyn CompletedWorkoutAdminUseCases>,
    ) -> Self {
        self.completed_workout_admin_service = Some(completed_workout_admin_service);
        self
    }

    pub fn with_workout_summary_service(
        mut self,
        workout_summary_service: Arc<dyn WorkoutSummaryUseCases>,
    ) -> Self {
        self.workout_summary_service = Some(workout_summary_service);
        self
    }

    pub fn with_athlete_summary_service(
        mut self,
        athlete_summary_service: Arc<dyn AthleteSummaryUseCases>,
    ) -> Self {
        self.athlete_summary_service = Some(athlete_summary_service);
        self
    }

    pub fn with_llm_services(
        mut self,
        llm_chat_service: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    ) -> Self {
        self.llm_chat_service = Some(llm_chat_service);
        self.llm_config_provider = Some(llm_config_provider);
        self
    }

    pub fn with_client_log_ingestion(mut self, enabled: bool) -> Self {
        self.client_log_ingestion_enabled = enabled;
        self
    }

    pub fn with_whitelist_rate_limiter(
        mut self,
        whitelist_rate_limiter: WhitelistRateLimiter,
    ) -> Self {
        self.whitelist_rate_limiter = whitelist_rate_limiter;
        self
    }

    pub fn with_intervals_service(mut self, intervals_service: Arc<dyn IntervalsUseCases>) -> Self {
        self.intervals_service = Some(intervals_service);
        self
    }

    pub fn with_race_service(mut self, race_service: Arc<dyn RaceUseCases>) -> Self {
        self.race_service = Some(race_service);
        self
    }

    pub fn with_intervals_connection_tester(
        mut self,
        intervals_connection_tester: Arc<dyn IntervalsConnectionTester>,
    ) -> Self {
        self.intervals_connection_tester = Some(intervals_connection_tester);
        self
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::WhitelistRateLimiter;

    #[test]
    fn whitelist_rate_limiter_prunes_empty_ip_buckets() {
        let limiter = WhitelistRateLimiter::new(1, Duration::from_millis(1));

        assert!(limiter.check("198.51.100.1"));
        std::thread::sleep(Duration::from_millis(5));
        assert!(limiter.check("198.51.100.2"));
        assert!(limiter.check("198.51.100.1"));
    }
}
