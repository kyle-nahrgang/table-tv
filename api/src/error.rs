use axum::{http::StatusCode, response::IntoResponse};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Unknown error occurred: {0}")]
    Unknown(String),

    #[error("Invalid server address: {0}")]
    InvalidAddress(#[from] std::net::AddrParseError),

    #[error("Server I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Camera not found")]
    CameraNotFound,

    #[error("Pool match not found")]
    PoolMatchNotFound,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Auth0 error: {0}")]
    Auth0ClientError(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::CameraNotFound => (StatusCode::NOT_FOUND, "Camera not found".to_string()),
            ApiError::PoolMatchNotFound => (StatusCode::NOT_FOUND, "Pool match not found".to_string()),
            ApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()),
            ApiError::Auth0ClientError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %message, "API returned 500");
        }
        (status, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn api_error_display() {
        assert_eq!(
            ApiError::CameraNotFound.to_string(),
            "Camera not found"
        );
        assert_eq!(
            ApiError::InvalidCredentials.to_string(),
            "Invalid credentials"
        );
        assert_eq!(
            ApiError::BadRequest("bad".to_string()).to_string(),
            "Bad request: bad"
        );
    }

    #[test]
    fn api_error_into_response_status_codes() {
        assert_eq!(
            ApiError::CameraNotFound.into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::PoolMatchNotFound.into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::InvalidCredentials.into_response().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ApiError::BadRequest("x".to_string()).into_response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::Forbidden("x".to_string()).into_response().status(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn api_error_from_io() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let api_err: ApiError = err.into();
        assert!(api_err.to_string().contains("file not found"));
    }
}
