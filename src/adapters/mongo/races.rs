use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::{
    intervals::DateRange,
    races::{
        BoxFuture as RaceBoxFuture, Race, RaceDiscipline, RaceError, RacePriority, RaceRepository,
        RaceResult,
    },
};

#[derive(Clone)]
pub struct MongoRaceRepository {
    collection: Collection<RaceDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RaceDocument {
    user_id: String,
    race_id: String,
    date: String,
    name: String,
    distance_meters: i32,
    discipline: String,
    priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoRaceRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client.database(database.as_ref()).collection("races"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), RaceError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "race_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("races_user_race_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("races_user_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| RaceError::Internal(error.to_string()))?;

        Ok(())
    }
}

impl RaceRepository for MongoRaceRepository {
    fn list_by_user_id(&self, user_id: &str) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                })
                .sort(doc! { "date": 1, "name": 1 })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .into_iter()
                .map(map_document_to_race)
                .collect()
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &range.oldest,
                        "$lte": &range.newest,
                    },
                })
                .sort(doc! { "date": 1, "name": 1 })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .into_iter()
                .map(map_document_to_race)
                .collect()
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> RaceBoxFuture<Result<Option<Race>, RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! { "user_id": &user_id, "race_id": &race_id })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .map(map_document_to_race)
                .transpose()
        })
    }

    fn upsert(&self, race: Race) -> RaceBoxFuture<Result<Race, RaceError>> {
        let collection = self.collection.clone();
        let document = map_race_to_document(&race);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! { "user_id": &document.user_id, "race_id": &document.race_id },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?;
            Ok(race)
        })
    }

    fn delete(&self, user_id: &str, race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! { "user_id": &user_id, "race_id": &race_id })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_race_to_document(race: &Race) -> RaceDocument {
    RaceDocument {
        user_id: race.user_id.clone(),
        race_id: race.race_id.clone(),
        date: race.date.clone(),
        name: race.name.clone(),
        distance_meters: race.distance_meters,
        discipline: race.discipline.as_str().to_string(),
        priority: race.priority.as_str().to_string(),
        result: race.result.as_ref().map(|r| r.as_str().to_string()),
        created_at_epoch_seconds: race.created_at_epoch_seconds,
        updated_at_epoch_seconds: race.updated_at_epoch_seconds,
    }
}

fn map_document_to_race(document: RaceDocument) -> Result<Race, RaceError> {
    Ok(Race {
        race_id: document.race_id,
        user_id: document.user_id,
        date: document.date,
        name: document.name,
        distance_meters: document.distance_meters,
        discipline: map_discipline(&document.discipline)?,
        priority: map_priority(&document.priority)?,
        result: document.result.as_deref().map(map_result).transpose()?,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}

fn map_discipline(value: &str) -> Result<RaceDiscipline, RaceError> {
    match value {
        "road" => Ok(RaceDiscipline::Road),
        "mtb" => Ok(RaceDiscipline::Mtb),
        "gravel" => Ok(RaceDiscipline::Gravel),
        "cyclocross" => Ok(RaceDiscipline::Cyclocross),
        "timetrial" => Ok(RaceDiscipline::Timetrial),
        other => Err(RaceError::Internal(format!(
            "unknown race discipline: {other}"
        ))),
    }
}

fn map_priority(value: &str) -> Result<RacePriority, RaceError> {
    match value {
        "A" => Ok(RacePriority::A),
        "B" => Ok(RacePriority::B),
        "C" => Ok(RacePriority::C),
        other => Err(RaceError::Internal(format!(
            "unknown race priority: {other}"
        ))),
    }
}

fn map_result(value: &str) -> Result<RaceResult, RaceError> {
    match value {
        "finished" => Ok(RaceResult::Finished),
        "dnf" => Ok(RaceResult::Dnf),
        "dsq" => Ok(RaceResult::Dsq),
        other => Err(RaceError::Internal(format!("unknown race result: {other}"))),
    }
}
