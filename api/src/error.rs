#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Unknown error occurred: {0}")]
    Unknown(String),

    #[error("Invalid server address: {0}")]
    InvalidAddress(#[from] std::net::AddrParseError),

    #[error("Server I/O error: {0}")]
    Io(#[from] std::io::Error),
}
