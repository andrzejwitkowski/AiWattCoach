use mongodb::{
    bson::{doc, oid::ObjectId},
    error::{ErrorKind, WriteFailure},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::identity::{
    AppUser, BoxFuture, GoogleIdentity, IdentityError, Role, UserRepository,
};

#[derive(Clone)]
pub struct MongoUserRepository {
    collection: Collection<UserDocument>,
}

impl MongoUserRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client.database(database.as_ref()).collection("app_users"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IdentityError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("app_users_user_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "google_subject": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("app_users_google_subject_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "email_normalized": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("app_users_email_normalized_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| IdentityError::Repository(error.to_string()))?;

        Ok(())
    }

    async fn find_one_by_identity(
        collection: &Collection<UserDocument>,
        google_subject: &str,
        email_normalized: &str,
    ) -> Result<Option<UserDocument>, IdentityError> {
        collection
            .find_one(doc! {
                "$or": [
                    { "google_subject": google_subject },
                    { "email_normalized": email_normalized },
                ]
            })
            .await
            .map_err(|error| IdentityError::Repository(error.to_string()))
    }
}

impl UserRepository for MongoUserRepository {
    fn find_by_id(&self, user_id: &str) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "user_id": &user_id })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_user_document))
        })
    }

    fn find_by_google_subject(
        &self,
        google_subject: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let collection = self.collection.clone();
        let google_subject = google_subject.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "google_subject": &google_subject })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_user_document))
        })
    }

    fn find_by_normalized_email(
        &self,
        normalized_email: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let collection = self.collection.clone();
        let normalized_email = normalized_email.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "email_normalized": &normalized_email })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_user_document))
        })
    }

    fn save(&self, user: AppUser) -> BoxFuture<Result<AppUser, IdentityError>> {
        let collection = self.collection.clone();
        let document = UserDocument::from_user(&user);
        Box::pin(async move {
            collection
                .replace_one(doc! { "user_id": &document.user_id }, &document)
                .upsert(true)
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(user)
        })
    }

    fn upsert_google_user(
        &self,
        new_user_id: String,
        google_identity: GoogleIdentity,
        roles: Vec<Role>,
    ) -> BoxFuture<Result<AppUser, IdentityError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let user = AppUser::new(
                new_user_id,
                google_identity.subject.clone(),
                google_identity.email.clone(),
                roles,
                google_identity.display_name.clone(),
                google_identity.avatar_url.clone(),
                google_identity.email_verified,
            );
            let document = UserDocument::from_user(&user);

            let filter = doc! {
                "$or": [
                    { "google_subject": &google_identity.subject },
                    { "email_normalized": &google_identity.email_normalized },
                ]
            };
            let update = doc! {
                "$set": {
                    "google_subject": &document.google_subject,
                    "email": &document.email,
                    "email_normalized": &document.email_normalized,
                    "email_verified": document.email_verified,
                    "display_name": &document.display_name,
                    "avatar_url": &document.avatar_url,
                    "roles": &document.roles,
                },
                "$setOnInsert": {
                    "user_id": &document.user_id,
                }
            };

            match collection
                .update_one(filter.clone(), update)
                .upsert(true)
                .await
            {
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind.as_ref(),
                        ErrorKind::Write(WriteFailure::WriteError(write_error)) if write_error.code == 11000
                    ) => {}
                Err(error) => return Err(IdentityError::Repository(error.to_string())),
            }

            let saved = Self::find_one_by_identity(
                &collection,
                &google_identity.subject,
                &google_identity.email_normalized,
            )
            .await?
            .ok_or_else(|| {
                IdentityError::Repository("upserted user missing after save".to_string())
            })?;

            Ok(map_user_document(saved))
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UserDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    user_id: String,
    google_subject: String,
    email: String,
    email_normalized: String,
    email_verified: bool,
    display_name: Option<String>,
    avatar_url: Option<String>,
    roles: Vec<String>,
}

impl UserDocument {
    fn from_user(user: &AppUser) -> Self {
        Self {
            id: None,
            user_id: user.id.clone(),
            google_subject: user.google_subject.clone(),
            email: user.email.clone(),
            email_normalized: user.email_normalized.clone(),
            email_verified: user.email_verified,
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
            roles: user
                .roles
                .iter()
                .map(|role| match role {
                    Role::User => "user".to_string(),
                    Role::Admin => "admin".to_string(),
                })
                .collect(),
        }
    }
}

fn map_user_document(document: UserDocument) -> AppUser {
    AppUser::new(
        document.user_id,
        document.google_subject,
        document.email,
        document
            .roles
            .into_iter()
            .filter_map(|role| match role.as_str() {
                "user" => Some(Role::User),
                "admin" => Some(Role::Admin),
                _ => None,
            })
            .collect(),
        document.display_name,
        document.avatar_url,
        document.email_verified,
    )
}
