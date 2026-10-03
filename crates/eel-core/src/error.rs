use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("floating-point values are not allowed in protocol objects")]
    FloatNotAllowed,
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("invalid identifier `{value}` for {kind}: {reason}")]
    InvalidId {
        kind: &'static str,
        value: String,
        reason: &'static str,
    },
    #[error("invalid hex hash: {0}")]
    InvalidHash(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
