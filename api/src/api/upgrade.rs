//! Upgrade endpoints: apt update and apt install table-tv.
//! Admin only. Streams command output to the client.

use axum::{
    body::Body,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::post,
};
use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::error::ApiError;

const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

/// POST /api/upgrade/check - Runs `apt update`. Streams output. Admin only.
pub async fn check_for_upgrades(
    auth: AuthenticatedUser,
    State(_app): State<AppState>,
) -> Result<Response, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }
    run_apt_stream(&["update"]).await
}

/// POST /api/upgrade/install - Runs `apt install -y table-tv`. Streams output. Admin only.
pub async fn upgrade_now(
    auth: AuthenticatedUser,
    State(_app): State<AppState>,
) -> Result<Response, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("Admin access required".to_string()));
    }
    run_apt_stream(&["install", "-y", PACKAGE_NAME]).await
}

async fn run_apt_stream(args: &[&str]) -> Result<Response, ApiError> {
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    tokio::spawn(async move {
        let mut child = match Command::new("apt")
            .args(&args)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(format!("Error spawning apt: {}\n", e)).await;
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };
        let stderr = match child.stderr.take() {
            Some(s) => s,
            None => return,
        };

        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stdout.send(line + "\n").await;
            }
        });

        let tx_stderr = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stderr.send(line + "\n").await;
            }
        });

        let _ = child.wait().await;
        drop(tx);
    });

    let stream = ReceiverStream::new(rx).map(|line| Ok::<_, std::io::Error>(Bytes::from(line)));
    let body = Body::from_stream(stream);

    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        body,
    )
        .into_response())
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/upgrade/check", post(check_for_upgrades))
        .route("/api/upgrade/install", post(upgrade_now))
}
