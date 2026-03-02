//! Recording download handler. Proxies MediaMTX playback to serve game clips.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::error::ApiError;

fn mediamtx_playback_base() -> String {
    std::env::var("MEDIAMTX_PLAYBACK_URL")
        .or_else(|_| std::env::var("MEDIAMTX_API_URL").map(|u| u.replace("9997", "9996")))
        .unwrap_or_else(|_| "http://127.0.0.1:9996".to_string())
}

#[derive(serde::Deserialize)]
pub struct RecordingDownloadQuery {
    /// Start time in milliseconds since epoch
    pub start: i64,
    /// Duration in seconds
    pub duration: f64,
}

/// GET /api/cameras/:id/recordings/download?start=...&duration=...
/// Proxies to MediaMTX playback server. Requires auth.
pub async fn recording_download(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<RecordingDownloadQuery>,
) -> Result<Response, ApiError> {
    if id.is_empty() || id.len() > 64 {
        return Err(ApiError::BadRequest("Invalid camera id".to_string()));
    }

    let _camera = app
        .db
        .find_camera_by_id(&id)?
        .ok_or(ApiError::CameraNotFound)?;

    if q.duration <= 0.0 || q.duration > 86400.0 {
        return Err(ApiError::BadRequest(
            "duration must be between 0 and 86400 seconds".to_string(),
        ));
    }

    let start_dt: DateTime<Utc> = DateTime::from_timestamp_millis(q.start)
        .ok_or_else(|| ApiError::BadRequest("Invalid start timestamp".to_string()))?;

    let path = format!("camera/{}", id);
    let start_rfc3339 = start_dt.to_rfc3339();
    let base = mediamtx_playback_base();
    let url = format!(
        "{}/get?path={}&start={}&duration={}&format=mp4",
        base,
        urlencoding::encode(&path),
        urlencoding::encode(&start_rfc3339),
        q.duration
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| ApiError::Unknown(e.to_string()))?;

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("Recording fetch failed: {}", e)))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        tracing::warn!(path = %path, status = %status, "Recording download failed: {}", body);
        return Err(ApiError::Unknown(format!(
            "Recording not available: {}",
            status
        )));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| ApiError::Unknown(format!("Recording stream failed: {}", e)))?;

    let filename = format!(
        "game-{}.mp4",
        start_dt.format("%Y%m%d-%H%M%S")
    );

    let content_disp = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"game.mp4\""));

    let response = (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("video/mp4")),
            (header::CONTENT_DISPOSITION, content_disp),
        ],
        bytes,
    )
        .into_response();

    Ok(response)
}
