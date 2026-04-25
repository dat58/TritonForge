//! Application-wide error type using thiserror for ergonomic error handling.

/// Unified error type for all application operations.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// File system or network I/O failure.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// User-supplied input failed validation.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Model conversion pipeline failure.
    #[error("Conversion failed: {0}")]
    Conversion(String),

    /// Docker daemon API error.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    /// SQLite database error.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Configuration file parsing error.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Config error: {0}")]
    Config(#[from] toml::de::Error),
}
