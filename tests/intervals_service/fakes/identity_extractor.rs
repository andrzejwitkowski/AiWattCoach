use aiwattcoach::domain::intervals::{
    ActivityFallbackIdentity, ActivityFileIdentityExtractorPort, IntervalsError, UploadActivity,
};

use crate::common::BoxFuture;

#[derive(Clone, Default)]
pub(crate) struct FakeActivityIdentityExtractor {
    identity: Option<ActivityFallbackIdentity>,
}

impl FakeActivityIdentityExtractor {
    pub(crate) fn with_identity(identity: ActivityFallbackIdentity) -> Self {
        Self {
            identity: Some(identity),
        }
    }
}

impl ActivityFileIdentityExtractorPort for FakeActivityIdentityExtractor {
    fn extract_identity(
        &self,
        _upload: &UploadActivity,
    ) -> BoxFuture<Result<Option<ActivityFallbackIdentity>, IntervalsError>> {
        let identity = self.identity.clone();
        Box::pin(async move { Ok(identity) })
    }
}
