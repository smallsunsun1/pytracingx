use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value as JsonValue;
use tracing::Level;

use crate::error::anyhow;
use crate::runtime;

#[pyclass(module = "pytracingx._native", name = "Logger")]
pub struct PyLogger {
    name: String,
}

impl PyLogger {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[pymethods]
impl PyLogger {
    #[pyo3(signature = (message, *, attributes = None))]
    fn trace(&self, message: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.emit(message, attributes, Level::TRACE, "TRACE")
    }

    #[pyo3(signature = (message, *, attributes = None))]
    fn debug(&self, message: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.emit(message, attributes, Level::DEBUG, "DEBUG")
    }

    #[pyo3(signature = (message, *, attributes = None))]
    fn info(&self, message: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.emit(message, attributes, Level::INFO, "INFO")
    }

    #[pyo3(signature = (message, *, attributes = None))]
    fn warn(&self, message: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.emit(message, attributes, Level::WARN, "WARN")
    }

    #[pyo3(signature = (message, *, attributes = None))]
    fn error(&self, message: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.emit(message, attributes, Level::ERROR, "ERROR")
    }

    #[pyo3(signature = (message, *, attributes = None))]
    fn fatal(&self, message: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.emit(message, attributes, Level::ERROR, "FATAL")
    }

    fn __repr__(&self) -> String {
        format!("Logger(name='{}')", self.name)
    }
}

impl PyLogger {
    fn emit(
        &self,
        message: String,
        attributes: Option<&Bound<'_, PyDict>>,
        level: Level,
        severity_text: &'static str,
    ) -> PyResult<()> {
        let attrs_json = serialize_attributes(attributes)?;
        emit_event(level, &self.name, severity_text, &message, attrs_json.as_deref());
        Ok(())
    }
}

#[pyfunction]
#[pyo3(signature = (name, message, *, severity_number, severity_text, attributes = None))]
pub fn emit_log_record(
    name: String,
    message: String,
    severity_number: i32,
    severity_text: String,
    attributes: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    if !runtime::is_initialized() {
        return Ok(());
    }
    let level = severity_to_level(severity_number);
    let attrs_json = serialize_attributes(attributes)?;
    let severity_text_static = leak_static(severity_text);
    emit_event(level, &name, severity_text_static, &message, attrs_json.as_deref());
    Ok(())
}

fn emit_event(
    level: Level,
    logger_name: &str,
    severity_text: &'static str,
    message: &str,
    attributes_json: Option<&str>,
) {
    let attrs = attributes_json.unwrap_or("");
    match level {
        Level::TRACE => tracing::trace!(
            target: "pytracingx::log",
            logger_name = logger_name,
            severity_text = severity_text,
            attributes = attrs,
            message,
        ),
        Level::DEBUG => tracing::debug!(
            target: "pytracingx::log",
            logger_name = logger_name,
            severity_text = severity_text,
            attributes = attrs,
            message,
        ),
        Level::INFO => tracing::info!(
            target: "pytracingx::log",
            logger_name = logger_name,
            severity_text = severity_text,
            attributes = attrs,
            message,
        ),
        Level::WARN => tracing::warn!(
            target: "pytracingx::log",
            logger_name = logger_name,
            severity_text = severity_text,
            attributes = attrs,
            message,
        ),
        Level::ERROR => tracing::error!(
            target: "pytracingx::log",
            logger_name = logger_name,
            severity_text = severity_text,
            attributes = attrs,
            message,
        ),
    }
}

#[pyfunction]
pub fn get_logger(name: String) -> PyResult<PyLogger> {
    if !runtime::is_initialized() {
        return Err(anyhow!("pytracingx is not initialized; call pytracingx.init(config) first").into());
    }
    Ok(PyLogger::new(name))
}

fn severity_to_level(value: i32) -> Level {
    match value {
        v if v <= 4 => Level::TRACE,
        v if v <= 8 => Level::DEBUG,
        v if v <= 12 => Level::INFO,
        v if v <= 16 => Level::WARN,
        _ => Level::ERROR,
    }
}

fn serialize_attributes(attrs: Option<&Bound<'_, PyDict>>) -> PyResult<Option<String>> {
    let Some(dict) = attrs else {
        return Ok(None);
    };
    if dict.is_empty() {
        return Ok(None);
    }
    let value: JsonValue = pythonize::depythonize(dict)?;
    Ok(Some(serde_json::to_string(&value).unwrap_or_default()))
}

fn leak_static(s: String) -> &'static str {
    match s.as_str() {
        "TRACE" => "TRACE",
        "DEBUG" => "DEBUG",
        "INFO" => "INFO",
        "WARN" => "WARN",
        "ERROR" => "ERROR",
        "FATAL" => "FATAL",
        _ => Box::leak(s.into_boxed_str()),
    }
}
