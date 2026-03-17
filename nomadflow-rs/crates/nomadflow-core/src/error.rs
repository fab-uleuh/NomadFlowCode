use thiserror::Error;

#[derive(Debug, Error)]
pub enum NomadError {
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NomadError>;
