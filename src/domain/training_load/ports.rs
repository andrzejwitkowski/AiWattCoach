use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{
    FtpHistoryEntry, TrainingLoadDailySnapshot, TrainingLoadError, TrainingLoadSnapshotRange,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait FtpHistoryRepository: Clone + Send + Sync + 'static {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>>;

    fn find_effective_for_date(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<Option<FtpHistoryEntry>, TrainingLoadError>>;

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> BoxFuture<Result<FtpHistoryEntry, TrainingLoadError>>;
}

pub trait TrainingLoadDailySnapshotRepository: Clone + Send + Sync + 'static {
    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &TrainingLoadSnapshotRange,
    ) -> BoxFuture<Result<Vec<TrainingLoadDailySnapshot>, TrainingLoadError>>;

    fn upsert(
        &self,
        snapshot: TrainingLoadDailySnapshot,
    ) -> BoxFuture<Result<TrainingLoadDailySnapshot, TrainingLoadError>>;

    fn delete_by_user_id_from_date(
        &self,
        user_id: &str,
        from_date: &str,
    ) -> BoxFuture<Result<(), TrainingLoadError>>;
}

#[derive(Clone, Default)]
pub struct NoopFtpHistoryRepository;

impl FtpHistoryRepository for NoopFtpHistoryRepository {
    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_effective_for_date(
        &self,
        _user_id: &str,
        _date: &str,
    ) -> BoxFuture<Result<Option<FtpHistoryEntry>, TrainingLoadError>> {
        Box::pin(async { Ok(None) })
    }

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> BoxFuture<Result<FtpHistoryEntry, TrainingLoadError>> {
        Box::pin(async move { Ok(entry) })
    }
}

#[derive(Clone, Default)]
pub struct NoopTrainingLoadDailySnapshotRepository;

impl TrainingLoadDailySnapshotRepository for NoopTrainingLoadDailySnapshotRepository {
    fn list_by_user_id_and_range(
        &self,
        _user_id: &str,
        _range: &TrainingLoadSnapshotRange,
    ) -> BoxFuture<Result<Vec<TrainingLoadDailySnapshot>, TrainingLoadError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        snapshot: TrainingLoadDailySnapshot,
    ) -> BoxFuture<Result<TrainingLoadDailySnapshot, TrainingLoadError>> {
        Box::pin(async move { Ok(snapshot) })
    }

    fn delete_by_user_id_from_date(
        &self,
        _user_id: &str,
        _from_date: &str,
    ) -> BoxFuture<Result<(), TrainingLoadError>> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct InMemoryFtpHistoryRepository {
    entries: Arc<Mutex<Vec<FtpHistoryEntry>>>,
}

#[cfg(test)]
impl InMemoryFtpHistoryRepository {
    pub fn stored(&self) -> Vec<FtpHistoryEntry> {
        let mut entries = self.entries.lock().unwrap().clone();
        entries.sort_by(|left, right| {
            left.user_id
                .cmp(&right.user_id)
                .then_with(|| left.effective_from_date.cmp(&right.effective_from_date))
        });
        entries
    }
}

#[cfg(test)]
impl FtpHistoryRepository for InMemoryFtpHistoryRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>> {
        let entries = self.entries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut values = entries
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            values.sort_by(|left, right| left.effective_from_date.cmp(&right.effective_from_date));
            Ok(values)
        })
    }

    fn find_effective_for_date(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<Option<FtpHistoryEntry>, TrainingLoadError>> {
        let entries = self.entries.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move {
            let mut values = entries
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id && entry.effective_from_date <= date)
                .cloned()
                .collect::<Vec<_>>();
            values.sort_by(|left, right| left.effective_from_date.cmp(&right.effective_from_date));
            Ok(values.into_iter().last())
        })
    }

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> BoxFuture<Result<FtpHistoryEntry, TrainingLoadError>> {
        let entries = self.entries.clone();
        Box::pin(async move {
            let mut entries = entries.lock().unwrap();
            entries.retain(|existing| {
                !(existing.user_id == entry.user_id
                    && existing.effective_from_date == entry.effective_from_date)
            });
            entries.push(entry.clone());
            Ok(entry)
        })
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct InMemoryTrainingLoadDailySnapshotRepository {
    snapshots: Arc<Mutex<Vec<TrainingLoadDailySnapshot>>>,
}

#[cfg(test)]
impl InMemoryTrainingLoadDailySnapshotRepository {
    pub fn stored(&self) -> Vec<TrainingLoadDailySnapshot> {
        let mut snapshots = self.snapshots.lock().unwrap().clone();
        snapshots.sort_by(|left, right| {
            left.user_id
                .cmp(&right.user_id)
                .then_with(|| left.date.cmp(&right.date))
        });
        snapshots
    }
}

#[cfg(test)]
impl TrainingLoadDailySnapshotRepository for InMemoryTrainingLoadDailySnapshotRepository {
    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &TrainingLoadSnapshotRange,
    ) -> BoxFuture<Result<Vec<TrainingLoadDailySnapshot>, TrainingLoadError>> {
        let snapshots = self.snapshots.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            let mut values = snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|snapshot| snapshot.user_id == user_id)
                .filter(|snapshot| snapshot.date >= oldest && snapshot.date <= newest)
                .cloned()
                .collect::<Vec<_>>();
            values.sort_by(|left, right| left.date.cmp(&right.date));
            Ok(values)
        })
    }

    fn upsert(
        &self,
        snapshot: TrainingLoadDailySnapshot,
    ) -> BoxFuture<Result<TrainingLoadDailySnapshot, TrainingLoadError>> {
        let snapshots = self.snapshots.clone();
        Box::pin(async move {
            let mut snapshots = snapshots.lock().unwrap();
            snapshots.retain(|existing| {
                !(existing.user_id == snapshot.user_id && existing.date == snapshot.date)
            });
            snapshots.push(snapshot.clone());
            Ok(snapshot)
        })
    }

    fn delete_by_user_id_from_date(
        &self,
        user_id: &str,
        from_date: &str,
    ) -> BoxFuture<Result<(), TrainingLoadError>> {
        let snapshots = self.snapshots.clone();
        let user_id = user_id.to_string();
        let from_date = from_date.to_string();
        Box::pin(async move {
            let mut snapshots = snapshots.lock().unwrap();
            snapshots
                .retain(|existing| !(existing.user_id == user_id && existing.date >= from_date));
            Ok(())
        })
    }
}
