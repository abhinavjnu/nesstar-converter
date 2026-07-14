use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum NesstarError {
    #[error("invalid source {path}: {reason}")]
    InvalidSource { path: PathBuf, reason: String },
    #[error("invalid DDI: {0}")]
    InvalidDdi(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("unsupported feature: {0}")]
    Unsupported(String),
}
