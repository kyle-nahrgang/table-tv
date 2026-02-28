pub mod api;
pub mod db;
pub mod error;
pub mod video;

#[tokio::main]
async fn main() -> Result<(), crate::error::ApiError> {
    api::ApiServer::serve().await
}
