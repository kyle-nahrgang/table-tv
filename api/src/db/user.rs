use polodb_core::bson::doc;
use polodb_core::CollectionT;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use super::Db;

const USERS_COLLECTION: &str = "users";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserDoc {
    pub auth0_sub: String,
    pub email: String,
    pub is_admin: bool,
}

impl Db {
    /// Returns true if at least one admin exists.
    pub fn has_admin(&self) -> Result<bool, ApiError> {
        let collection = self.0.collection::<UserDoc>(USERS_COLLECTION);
        let admin = collection.find_one(doc! { "is_admin": true })?;
        Ok(admin.is_some())
    }

    /// Find user by Auth0 sub (subject) claim.
    pub fn find_user_by_sub(&self, sub: &str) -> Result<Option<UserDoc>, ApiError> {
        let collection = self.0.collection::<UserDoc>(USERS_COLLECTION);
        let user = collection.find_one(doc! { "auth0_sub": sub })?;
        Ok(user)
    }

    /// Create or update a user. First user becomes admin.
    pub fn upsert_user(&self, auth0_sub: String, email: String) -> Result<UserDoc, ApiError> {
        let collection = self.0.collection::<UserDoc>(USERS_COLLECTION);
        if let Some(existing) = collection.find_one(doc! { "auth0_sub": &auth0_sub })? {
            return Ok(existing);
        }

        let is_admin = !self.has_admin()?;
        let user = UserDoc {
            auth0_sub: auth0_sub.clone(),
            email: email.clone(),
            is_admin,
        };
        collection.insert_one(user.clone())?;
        Ok(user)
    }
}
