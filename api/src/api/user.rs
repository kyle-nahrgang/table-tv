use axum::{
    extract::{Path, State},
    routing::{get, patch},
    Json,
};
use serde::Deserialize;

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::db::user::UserDoc;
use crate::error::ApiError;

/// GET /api/users - List all users (admin only).
pub async fn list_users(
    auth: AuthenticatedUser,
    State(app): State<AppState>,
) -> Result<Json<Vec<UserDoc>>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }
    let users = app.db.list_users()?;
    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub is_admin: Option<bool>,
}

/// PATCH /api/users/:sub - Update user (admin only). Supports is_admin.
pub async fn update_user(
    auth: AuthenticatedUser,
    Path(sub): Path<String>,
    State(app): State<AppState>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserDoc>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }
    let is_admin = req
        .is_admin
        .ok_or_else(|| ApiError::BadRequest("is_admin is required".to_string()))?;
    let user = app.db.set_user_admin(&sub, is_admin)?;
    Ok(Json(user))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users/:sub", patch(update_user))
}
