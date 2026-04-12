use std::{future::Future, pin::Pin};

use super::{
    CanonicalEntityRef, ExternalObservation, ExternalProvider, ExternalSyncState,
    ProviderPollState, ProviderPollStream,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait ExternalObservationRepository: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        observation: ExternalObservation,
    ) -> BoxFuture<Result<ExternalObservation, std::convert::Infallible>>;

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> BoxFuture<Result<Option<ExternalObservation>, std::convert::Infallible>>;
}

pub trait ExternalSyncStateRepository: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> BoxFuture<Result<ExternalSyncState, std::convert::Infallible>>;

    fn find_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, std::convert::Infallible>>;

    fn delete_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<(), std::convert::Infallible>>;
}

pub trait ProviderPollStateRepository: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> BoxFuture<Result<ProviderPollState, std::convert::Infallible>>;

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<Vec<ProviderPollState>, std::convert::Infallible>>;

    fn find_by_provider_and_stream(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        stream: ProviderPollStream,
    ) -> BoxFuture<Result<Option<ProviderPollState>, std::convert::Infallible>>;
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct NoopExternalObservationRepository;

#[cfg(test)]
impl ExternalObservationRepository for NoopExternalObservationRepository {
    fn upsert(
        &self,
        observation: ExternalObservation,
    ) -> BoxFuture<Result<ExternalObservation, std::convert::Infallible>> {
        Box::pin(async move { Ok(observation) })
    }

    fn find_by_provider_and_external_id(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _external_id: &str,
    ) -> BoxFuture<Result<Option<ExternalObservation>, std::convert::Infallible>> {
        Box::pin(async { Ok(None) })
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct NoopExternalSyncStateRepository;

#[cfg(test)]
impl ExternalSyncStateRepository for NoopExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> BoxFuture<Result<ExternalSyncState, std::convert::Infallible>> {
        Box::pin(async move { Ok(state) })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, std::convert::Infallible>> {
        Box::pin(async { Ok(None) })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<(), std::convert::Infallible>> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopProviderPollStateRepository;

#[cfg(test)]
impl ProviderPollStateRepository for NoopProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> BoxFuture<Result<ProviderPollState, std::convert::Infallible>> {
        Box::pin(async move { Ok(state) })
    }

    fn list_due(
        &self,
        _now_epoch_seconds: i64,
    ) -> BoxFuture<Result<Vec<ProviderPollState>, std::convert::Infallible>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_by_provider_and_stream(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _stream: ProviderPollStream,
    ) -> BoxFuture<Result<Option<ProviderPollState>, std::convert::Infallible>> {
        Box::pin(async { Ok(None) })
    }
}
