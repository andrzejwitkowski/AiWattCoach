mod bootstrap;
mod documents;
mod mapping;
mod repository;

#[cfg(test)]
mod tests;

use mongodb::{bson::doc, options::IndexOptions, IndexModel};

use crate::domain::settings::SettingsError;

pub use bootstrap::IntervalsPollBootstrapUser;
pub use repository::MongoUserSettingsRepository;

impl MongoUserSettingsRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("user_settings"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), SettingsError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("user_settings_user_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "wahoo.user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("user_settings_wahoo_user_id_unique".to_string())
                            .unique(true)
                            .partial_filter_expression(doc! {
                                "wahoo.user_id": { "$type": ["long", "int"] }
                            })
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|e| SettingsError::Repository(e.to_string()))?;
        Ok(())
    }
}
