use axum::{extract::State, routing::get, Router};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api::{admin, camera, info, pool_match};
use crate::db::Db;
use crate::error::ApiError;
use crate::video::OverlayState;

pub struct ApiServer;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub overlay: OverlayState,
}

impl ApiServer {
    fn router(db: Db) -> Router {
        let overlay: OverlayState = Arc::new(RwLock::new(None));
        let app_state = AppState {
            db: db.clone(),
            overlay: overlay.clone(),
        };

        let mut app = Router::new()
            .route("/api/hello", get(hello_world))
            .merge(admin::routes())
            .merge(camera::routes())
            .merge(pool_match::routes())
            .merge(info::routes())
            .layer(TraceLayer::new_for_http())
            .with_state(app_state);

        if Path::new("ui-dist").exists() {
            let serve_dir = ServeDir::new("ui-dist")
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new("ui-dist/index.html"));
            app = app.nest_service("/", serve_dir);
        }

        app
    }

    pub async fn serve(db: Db) -> Result<(), ApiError> {
        let app = Self::router(db);
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        tracing::info!("starting api server");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn hello_world(State(_app): State<AppState>) -> &'static str {
    "hello world"
}
