use axum::{extract::State, routing::post, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::ApiError;

#[derive(Deserialize)]
pub struct AdminCreateRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct AdminLoginRequest {
    pub email: String,
    pub password: String,
}

/// POST /api/admin - Create the first admin. Fails if any admin already exists.
pub async fn admin_create(
    State(app): State<AppState>,
    Json(req): Json<AdminCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.email.is_empty() || req.password.is_empty() {
        return Err(ApiError::BadRequest("email and password required".to_string()));
    }
    let hash = bcrypt::hash(req.password, bcrypt::DEFAULT_COST)
        .map_err(|e| ApiError::Unknown(e.to_string()))?;
    app.db.create_admin(req.email, hash)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/admin/login - Authenticate admin by email and password.
pub async fn admin_login(
    State(app): State<AppState>,
    Json(req): Json<AdminLoginRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let admin = app.db
        .find_admin_by_email(&req.email)?
        .ok_or(ApiError::InvalidCredentials)?;
    let valid = bcrypt::verify(req.password, &admin.hash)
        .map_err(|e| ApiError::Unknown(e.to_string()))?;
    if !valid {
        return Err(ApiError::InvalidCredentials);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/admin", post(admin_create))
        .route("/api/admin/login", post(admin_login))
}
