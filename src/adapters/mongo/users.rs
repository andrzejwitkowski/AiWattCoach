use mongodb::{
    bson::{doc, oid::ObjectId},
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

    async fn find_one_by_google_subject(
        collection: &Collection<UserDocument>,
        google_subject: &str,
    ) -> Result<Option<UserDocument>, IdentityError> {
        collection
            .find_one(doc! { "google_subject": google_subject })
            .await
            .map_err(|error| IdentityError::Repository(error.to_string()))
    }

    async fn find_one_by_normalized_email(
        collection: &Collection<UserDocument>,
        email_normalized: &str,
    ) -> Result<Option<UserDocument>, IdentityError> {
        collection
            .find_one(doc! { "email_normalized": email_normalized })
            .await
            .map_err(|error| IdentityError::Repository(error.to_string()))
    }

    fn build_user_document(
        new_user_id: String,
        google_identity: &GoogleIdentity,
        roles: Vec<Role>,
    ) -> UserDocument {
        UserDocument::from_user(&AppUser::new(
            new_user_id,
            google_identity.subject.clone(),
            google_identity.email.clone(),
            roles,
            google_identity.display_name.clone(),
            google_identity.avatar_url.clone(),
            google_identity.email_verified,
        ))
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
            let document =
                Self::find_one_by_normalized_email(&collection, &normalized_email).await?;

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
            let document = Self::build_user_document(new_user_id, &google_identity, roles);
            let filter = doc! { "google_subject": &google_identity.subject };
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

            collection
                .update_one(filter.clone(), update)
                .upsert(true)
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            let saved = Self::find_one_by_google_subject(&collection, &google_identity.subject)
                .await?
                .ok_or_else(|| {
                    IdentityError::Repository("upserted user missing after save".to_string())
                })?;

            Ok(map_user_document(saved))
        })
    }

    fn save_google_user_for_identity(
        &self,
        new_user_id: String,
        google_identity: GoogleIdentity,
        roles: Vec<Role>,
    ) -> BoxFuture<Result<AppUser, IdentityError>> {
        let repository = self.clone();
        Box::pin(async move {
            let by_subject = repository
                .find_by_google_subject(&google_identity.subject)
                .await?;
            let by_email = repository
                .find_by_normalized_email(&google_identity.email_normalized)
                .await?;

            match (by_subject, by_email) {
                (Some(subject_user), Some(email_user)) if subject_user.id != email_user.id => {
                    Err(IdentityError::Repository(
                        "conflicting google subject/email mapping".to_string(),
                    ))
                }
                (Some(_), _) => {
                    repository
                        .upsert_google_user(new_user_id, google_identity, roles)
                        .await
                }
                (None, Some(email_user)) => {
                    if !email_user.google_subject.is_empty()
                        && email_user.google_subject != google_identity.subject
                    {
                        return Err(IdentityError::Repository(
                            "conflicting google subject/email mapping".to_string(),
                        ));
                    }

                    repository
                        .save(AppUser::new(
                            email_user.id,
                            google_identity.subject,
                            google_identity.email,
                            roles,
                            google_identity.display_name,
                            google_identity.avatar_url,
                            google_identity.email_verified,
                        ))
                        .await
                }
                (None, None) => {
                    repository
                        .upsert_google_user(new_user_id, google_identity, roles)
                        .await
                }
            }
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
