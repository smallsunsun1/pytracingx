use thiserror::Error;

#[derive(Debug, Error)]
pub enum PtxError {
    #[error("pytracingx is not initialized; call pytracingx.init(config) first")]
    NotInitialized,

    #[error("pytracingx is already initialized; call pytracingx.shutdown() before re-initializing")]
    AlreadyInitialized,

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("invalid endpoint URL: {0}")]
    Endpoint(String),

    #[error("OTLP exporter build failed: {0}")]
    Exporter(String),

    #[error("internal runtime error: {0}")]
    Runtime(String),
}

impl From<PtxError> for pyo3::PyErr {
    fn from(value: PtxError) -> Self {
        use pyo3::exceptions::{PyRuntimeError, PyValueError};
        match value {
            PtxError::Config(_) | PtxError::Endpoint(_) => PyValueError::new_err(value.to_string()),
            other => PyRuntimeError::new_err(other.to_string()),
        }
    }
}

pub type PtxResult<T> = Result<T, PtxError>;
