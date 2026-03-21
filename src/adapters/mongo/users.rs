use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection,
};
use serde::{Deserialize, Serialize};

use crate::domain::identity::{AppUser, BoxFuture, IdentityError, Role, UserRepository};

#[derive(Clone)]
pub struct MongoUserRepository {
    collection: Collection<UserDocument>,
}

impl MongoUserRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("app_users"),
        }
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

    fn find_by_google_subject(&self, google_subject: &str) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
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

    fn find_by_normalized_email(&self, normalized_email: &str) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
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
