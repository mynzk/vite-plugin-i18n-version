use thiserror::Error;

#[derive(Debug, Error)]
pub enum I18nVersionError {
    #[error("root path does not exist: {0}")]
    RootNotFound(String),

    #[error("failed to scan directory {path}: {source}")]
    ScanError {
        path: String,
        #[source]
        source: walkdir::Error,
    },

    #[error("failed to read file {path}: {source}")]
    ReadError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid glob pattern {pattern}: {source}")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("invalid JSON in {path}: {message}")]
    InvalidJson { path: String, message: String },

    #[error("length must be in [1, 64], got {0}")]
    InvalidLength(usize),
}