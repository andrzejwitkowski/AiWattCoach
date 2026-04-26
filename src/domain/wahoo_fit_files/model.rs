use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WahooFitFileStage {
    Queued,
    Downloaded,
    Stored,
    Parsed,
    Enriched,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WahooFitFileError {
    Repository(String),
}

impl std::fmt::Display for WahooFitFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WahooFitFileError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooFitFile {
    pub user_id: String,
    pub completed_workout_id: String,
    pub wahoo_workout_id: i64,
    pub stage: WahooFitFileStage,
    pub file_url: Option<String>,
    pub file_hash_sha256: Option<String>,
    pub raw_fit_bytes: Option<Vec<u8>>,
    pub downloaded_at_epoch_seconds: Option<i64>,
    pub stored_at_epoch_seconds: Option<i64>,
    pub parsed_at_epoch_seconds: Option<i64>,
    pub enriched_at_epoch_seconds: Option<i64>,
    pub updated_at_epoch_seconds: i64,
}

impl WahooFitFile {
    pub fn new(
        user_id: String,
        completed_workout_id: String,
        wahoo_workout_id: i64,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            completed_workout_id,
            wahoo_workout_id,
            stage: WahooFitFileStage::Queued,
            file_url: None,
            file_hash_sha256: None,
            raw_fit_bytes: None,
            downloaded_at_epoch_seconds: None,
            stored_at_epoch_seconds: None,
            parsed_at_epoch_seconds: None,
            enriched_at_epoch_seconds: None,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn mark_queued(mut self, now_epoch_seconds: i64) -> Self {
        self.stage = WahooFitFileStage::Queued;
        self.updated_at_epoch_seconds = now_epoch_seconds;
        self
    }

    pub fn mark_downloaded(mut self, file_url: String, now_epoch_seconds: i64) -> Self {
        self.stage = WahooFitFileStage::Downloaded;
        self.file_url = Some(file_url);
        self.downloaded_at_epoch_seconds = Some(now_epoch_seconds);
        self.updated_at_epoch_seconds = now_epoch_seconds;
        self
    }

    pub fn mark_stored(
        mut self,
        file_url: String,
        file_hash_sha256: String,
        raw_fit_bytes: Vec<u8>,
        now_epoch_seconds: i64,
    ) -> Self {
        self.stage = WahooFitFileStage::Stored;
        self.file_url = Some(file_url);
        self.file_hash_sha256 = Some(file_hash_sha256);
        self.raw_fit_bytes = Some(raw_fit_bytes);
        self.downloaded_at_epoch_seconds
            .get_or_insert(now_epoch_seconds);
        self.stored_at_epoch_seconds = Some(now_epoch_seconds);
        self.updated_at_epoch_seconds = now_epoch_seconds;
        self
    }

    pub fn mark_parsed(mut self, now_epoch_seconds: i64) -> Self {
        self.stage = WahooFitFileStage::Parsed;
        self.parsed_at_epoch_seconds = Some(now_epoch_seconds);
        self.updated_at_epoch_seconds = now_epoch_seconds;
        self
    }

    pub fn mark_enriched(mut self, now_epoch_seconds: i64) -> Self {
        self.stage = WahooFitFileStage::Enriched;
        self.enriched_at_epoch_seconds = Some(now_epoch_seconds);
        self.updated_at_epoch_seconds = now_epoch_seconds;
        self
    }
}
