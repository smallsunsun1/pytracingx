use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::error::{PtxError, PtxResult};

const DEFAULT_BATCH_QUEUE: usize = 2_048;
const DEFAULT_BATCH_EXPORT: usize = 512;
const DEFAULT_BATCH_DELAY_MS: u64 = 5_000;
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_METRIC_INTERVAL_MS: u64 = 60_000;

// ─── Protocol / Sampler enums ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Grpc,
    HttpProtobuf,
}

impl Protocol {
    pub fn parse(value: &str) -> PtxResult<Self> {
        match value {
            "grpc" => Ok(Protocol::Grpc),
            "http/protobuf" | "http" => Ok(Protocol::HttpProtobuf),
            other => Err(PtxError::Config(format!(
                "unknown protocol '{other}', expected 'grpc' or 'http/protobuf'"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Grpc => "grpc",
            Protocol::HttpProtobuf => "http/protobuf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampler {
    AlwaysOn,
    AlwaysOff,
    ParentBasedTraceIdRatio,
}

impl Sampler {
    pub fn parse(value: &str) -> PtxResult<Self> {
        match value {
            "always_on" => Ok(Sampler::AlwaysOn),
            "always_off" => Ok(Sampler::AlwaysOff),
            "parent_based_traceid_ratio" => Ok(Sampler::ParentBasedTraceIdRatio),
            other => Err(PtxError::Config(format!(
                "unknown sampler '{other}', expected 'always_on' | 'always_off' | 'parent_based_traceid_ratio'"
            ))),
        }
    }
}

// ─── Sink pyclasses ──────────────────────────────────────────────────────────

#[pyclass(module = "pytracingx._native", name = "TraceSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTraceSink {
    pub(crate) endpoint: String,
    pub(crate) protocol: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) timeout_ms: u64,
    pub(crate) sampler: String,
    pub(crate) sampler_arg: f64,
    pub(crate) batch_max_queue: usize,
    pub(crate) batch_max_export: usize,
    pub(crate) batch_schedule_delay_ms: u64,
}

#[pymethods]
impl PyTraceSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        protocol = "grpc".to_string(),
        headers = None,
        timeout_ms = DEFAULT_TIMEOUT_MS,
        sampler = "parent_based_traceid_ratio".to_string(),
        sampler_arg = 1.0,
        batch_max_queue = DEFAULT_BATCH_QUEUE,
        batch_max_export = DEFAULT_BATCH_EXPORT,
        batch_schedule_delay_ms = DEFAULT_BATCH_DELAY_MS,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        endpoint: String,
        protocol: String,
        headers: Option<HashMap<String, String>>,
        timeout_ms: u64,
        sampler: String,
        sampler_arg: f64,
        batch_max_queue: usize,
        batch_max_export: usize,
        batch_schedule_delay_ms: u64,
    ) -> PyResult<Self> {
        Protocol::parse(&protocol)?;
        Sampler::parse(&sampler)?;
        if endpoint.is_empty() {
            return Err(PtxError::Config("TraceSink endpoint must not be empty".into()).into());
        }
        Ok(Self {
            endpoint,
            protocol,
            headers: headers.unwrap_or_default(),
            timeout_ms,
            sampler,
            sampler_arg,
            batch_max_queue,
            batch_max_export,
            batch_schedule_delay_ms,
        })
    }
}

#[pyclass(module = "pytracingx._native", name = "MetricSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMetricSink {
    pub(crate) endpoint: String,
    pub(crate) protocol: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) timeout_ms: u64,
    pub(crate) export_interval_ms: u64,
}

#[pymethods]
impl PyMetricSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        protocol = "grpc".to_string(),
        headers = None,
        timeout_ms = DEFAULT_TIMEOUT_MS,
        export_interval_ms = DEFAULT_METRIC_INTERVAL_MS,
    ))]
    fn new(
        endpoint: String,
        protocol: String,
        headers: Option<HashMap<String, String>>,
        timeout_ms: u64,
        export_interval_ms: u64,
    ) -> PyResult<Self> {
        Protocol::parse(&protocol)?;
        if endpoint.is_empty() {
            return Err(PtxError::Config("MetricSink endpoint must not be empty".into()).into());
        }
        Ok(Self {
            endpoint,
            protocol,
            headers: headers.unwrap_or_default(),
            timeout_ms,
            export_interval_ms,
        })
    }
}

#[pyclass(module = "pytracingx._native", name = "OtlpLogSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyOtlpLogSink {
    pub(crate) endpoint: String,
    pub(crate) protocol: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) timeout_ms: u64,
    pub(crate) batch_max_queue: usize,
    pub(crate) batch_max_export: usize,
    pub(crate) batch_schedule_delay_ms: u64,
}

#[pymethods]
impl PyOtlpLogSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        protocol = "grpc".to_string(),
        headers = None,
        timeout_ms = DEFAULT_TIMEOUT_MS,
        batch_max_queue = DEFAULT_BATCH_QUEUE,
        batch_max_export = DEFAULT_BATCH_EXPORT,
        batch_schedule_delay_ms = DEFAULT_BATCH_DELAY_MS,
    ))]
    fn new(
        endpoint: String,
        protocol: String,
        headers: Option<HashMap<String, String>>,
        timeout_ms: u64,
        batch_max_queue: usize,
        batch_max_export: usize,
        batch_schedule_delay_ms: u64,
    ) -> PyResult<Self> {
        Protocol::parse(&protocol)?;
        if endpoint.is_empty() {
            return Err(PtxError::Config("OtlpLogSink endpoint must not be empty".into()).into());
        }
        Ok(Self {
            endpoint,
            protocol,
            headers: headers.unwrap_or_default(),
            timeout_ms,
            batch_max_queue,
            batch_max_export,
            batch_schedule_delay_ms,
        })
    }
}

#[pyclass(module = "pytracingx._native", name = "SlsLogSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PySlsLogSink {
    pub(crate) endpoint: String,
    pub(crate) project: String,
    pub(crate) logstore: String,
    pub(crate) ak_id: String,
    pub(crate) ak_secret: String,
    pub(crate) topic: String,
    pub(crate) source: String,
}

#[pymethods]
impl PySlsLogSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        project,
        logstore,
        ak_id,
        ak_secret,
        topic = "".to_string(),
        source = "".to_string(),
    ))]
    fn new(
        endpoint: String,
        project: String,
        logstore: String,
        ak_id: String,
        ak_secret: String,
        topic: String,
        source: String,
    ) -> PyResult<Self> {
        if endpoint.is_empty() {
            return Err(PtxError::Config("SlsLogSink endpoint must not be empty".into()).into());
        }
        if project.is_empty() {
            return Err(PtxError::Config("SlsLogSink project must not be empty".into()).into());
        }
        if logstore.is_empty() {
            return Err(PtxError::Config("SlsLogSink logstore must not be empty".into()).into());
        }
        if ak_id.is_empty() || ak_secret.is_empty() {
            return Err(PtxError::Config("SlsLogSink ak_id and ak_secret must not be empty".into()).into());
        }
        Ok(Self {
            endpoint,
            project,
            logstore,
            ak_id,
            ak_secret,
            topic,
            source,
        })
    }
}

// ─── Resolved config types (Rust-internal) ───────────────────────────────────

#[derive(Debug, Clone)]
pub struct ResolvedTraceSink {
    pub endpoint: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, String>,
    pub timeout_ms: u64,
    pub sampler: Sampler,
    pub sampler_arg: f64,
    pub batch_max_queue: usize,
    pub batch_max_export: usize,
    pub batch_schedule_delay_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ResolvedMetricSink {
    pub endpoint: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, String>,
    pub timeout_ms: u64,
    pub export_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ResolvedOtlpLogSink {
    pub endpoint: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, String>,
    pub timeout_ms: u64,
    pub batch_max_queue: usize,
    pub batch_max_export: usize,
    pub batch_schedule_delay_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ResolvedSlsLogSink {
    pub endpoint: String,
    pub project: String,
    pub logstore: String,
    pub ak_id: String,
    pub ak_secret: String,
    pub topic: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub service_name: String,
    pub resource_attributes: HashMap<String, String>,
    pub console_output: bool,
    pub console_level: String,
    pub console_format: String,
    pub log_filter: Option<String>,
    pub traces: Option<ResolvedTraceSink>,
    pub metrics: Option<ResolvedMetricSink>,
    pub otlp_logs: Option<ResolvedOtlpLogSink>,
    pub sls_log: Option<ResolvedSlsLogSink>,
}

/// Borrowed view of transport params, used by exporter builders.
pub struct SignalTransport<'a> {
    pub endpoint: &'a str,
    pub protocol: Protocol,
    pub headers: &'a HashMap<String, String>,
    pub timeout_ms: u64,
}

// ─── PyConfig ────────────────────────────────────────────────────────────────

#[pyclass(module = "pytracingx._native", name = "Config", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyConfig {
    pub(crate) service_name: String,
    pub(crate) resource_attributes: HashMap<String, String>,
    pub(crate) console_output: bool,
    pub(crate) console_level: String,
    pub(crate) console_format: String,
    pub(crate) log_filter: Option<String>,
    pub(crate) trace_sink: Option<PyTraceSink>,
    pub(crate) metric_sink: Option<PyMetricSink>,
    pub(crate) otlp_log_sink: Option<PyOtlpLogSink>,
    pub(crate) sls_log_sink: Option<PySlsLogSink>,
}

#[pymethods]
impl PyConfig {
    #[new]
    #[pyo3(signature = (
        service_name,
        resource_attributes = None,
        console_output = true,
        console_level = "info".to_string(),
        console_format = "compact".to_string(),
        log_filter = None,
        sinks = None,
    ))]
    fn new(
        service_name: String,
        resource_attributes: Option<HashMap<String, String>>,
        console_output: bool,
        console_level: String,
        console_format: String,
        log_filter: Option<String>,
        sinks: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<Self> {
        if service_name.is_empty() {
            return Err(PtxError::Config("service_name must not be empty".into()).into());
        }
        validate_console_format(&console_format)?;

        let mut trace_sink: Option<PyTraceSink> = None;
        let mut metric_sink: Option<PyMetricSink> = None;
        let mut otlp_log_sink: Option<PyOtlpLogSink> = None;
        let mut sls_log_sink: Option<PySlsLogSink> = None;

        if let Some(sink_list) = sinks {
            for obj in sink_list {
                if let Ok(s) = obj.extract::<PyTraceSink>() {
                    if trace_sink.is_some() {
                        return Err(PtxError::Config("only one TraceSink allowed".into()).into());
                    }
                    trace_sink = Some(s);
                } else if let Ok(s) = obj.extract::<PyMetricSink>() {
                    if metric_sink.is_some() {
                        return Err(PtxError::Config("only one MetricSink allowed".into()).into());
                    }
                    metric_sink = Some(s);
                } else if let Ok(s) = obj.extract::<PyOtlpLogSink>() {
                    if otlp_log_sink.is_some() {
                        return Err(PtxError::Config("only one OtlpLogSink allowed".into()).into());
                    }
                    otlp_log_sink = Some(s);
                } else if let Ok(s) = obj.extract::<PySlsLogSink>() {
                    if sls_log_sink.is_some() {
                        return Err(PtxError::Config("only one SlsLogSink allowed".into()).into());
                    }
                    sls_log_sink = Some(s);
                } else {
                    return Err(PtxError::Config(
                        "sinks list must contain TraceSink, MetricSink, OtlpLogSink, or SlsLogSink".into(),
                    ).into());
                }
            }
        }

        Ok(Self {
            service_name,
            resource_attributes: resource_attributes.unwrap_or_default(),
            console_output,
            console_level,
            console_format,
            log_filter,
            trace_sink,
            metric_sink,
            otlp_log_sink,
            sls_log_sink,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Config(service_name='{}', traces={}, metrics={}, otlp_logs={}, sls_log={}, console={})",
            self.service_name,
            self.trace_sink.is_some(),
            self.metric_sink.is_some(),
            self.otlp_log_sink.is_some(),
            self.sls_log_sink.is_some(),
            self.console_output,
        )
    }

    fn describe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("service_name", &self.service_name)?;
        dict.set_item("console_output", self.console_output)?;
        dict.set_item("console_level", &self.console_level)?;
        dict.set_item("console_format", &self.console_format)?;
        dict.set_item("log_filter", self.log_filter.clone())?;
        dict.set_item("traces", self.trace_sink.is_some())?;
        dict.set_item("metrics", self.metric_sink.is_some())?;
        dict.set_item("otlp_logs", self.otlp_log_sink.is_some())?;
        dict.set_item("sls_log", self.sls_log_sink.is_some())?;
        Ok(dict)
    }
}

impl PyConfig {
    pub fn resolve(&self) -> PtxResult<ResolvedConfig> {
        validate_console_format(&self.console_format)
            .map_err(|e| PtxError::Config(e.to_string()))?;

        let traces = self.trace_sink.as_ref().map(|s| -> PtxResult<ResolvedTraceSink> {
            Ok(ResolvedTraceSink {
                endpoint: s.endpoint.clone(),
                protocol: Protocol::parse(&s.protocol)?,
                headers: s.headers.clone(),
                timeout_ms: s.timeout_ms,
                sampler: Sampler::parse(&s.sampler)?,
                sampler_arg: s.sampler_arg,
                batch_max_queue: s.batch_max_queue,
                batch_max_export: s.batch_max_export,
                batch_schedule_delay_ms: s.batch_schedule_delay_ms,
            })
        }).transpose()?;

        let metrics = self.metric_sink.as_ref().map(|s| -> PtxResult<ResolvedMetricSink> {
            Ok(ResolvedMetricSink {
                endpoint: s.endpoint.clone(),
                protocol: Protocol::parse(&s.protocol)?,
                headers: s.headers.clone(),
                timeout_ms: s.timeout_ms,
                export_interval_ms: s.export_interval_ms,
            })
        }).transpose()?;

        let otlp_logs = self.otlp_log_sink.as_ref().map(|s| -> PtxResult<ResolvedOtlpLogSink> {
            Ok(ResolvedOtlpLogSink {
                endpoint: s.endpoint.clone(),
                protocol: Protocol::parse(&s.protocol)?,
                headers: s.headers.clone(),
                timeout_ms: s.timeout_ms,
                batch_max_queue: s.batch_max_queue,
                batch_max_export: s.batch_max_export,
                batch_schedule_delay_ms: s.batch_schedule_delay_ms,
            })
        }).transpose()?;

        let sls_log = self.sls_log_sink.as_ref().map(|s| {
            ResolvedSlsLogSink {
                endpoint: s.endpoint.clone(),
                project: s.project.clone(),
                logstore: s.logstore.clone(),
                ak_id: s.ak_id.clone(),
                ak_secret: s.ak_secret.clone(),
                topic: s.topic.clone(),
                source: if s.source.is_empty() {
                    hostname::get()
                        .map(|h| h.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    s.source.clone()
                },
            }
        });

        Ok(ResolvedConfig {
            service_name: self.service_name.clone(),
            resource_attributes: self.resource_attributes.clone(),
            console_output: self.console_output,
            console_level: self.console_level.clone(),
            console_format: self.console_format.clone(),
            log_filter: self.log_filter.clone(),
            traces,
            metrics,
            otlp_logs,
            sls_log,
        })
    }
}

fn validate_console_format(fmt: &str) -> PyResult<()> {
    match fmt {
        "compact" | "pretty" | "json" => Ok(()),
        other => Err(PtxError::Config(format!(
            "unknown console_format '{other}', expected 'compact' | 'pretty' | 'json'"
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_parse_valid() {
        assert_eq!(Protocol::parse("grpc").unwrap(), Protocol::Grpc);
        assert_eq!(Protocol::parse("http/protobuf").unwrap(), Protocol::HttpProtobuf);
        assert_eq!(Protocol::parse("http").unwrap(), Protocol::HttpProtobuf);
    }

    #[test]
    fn protocol_parse_invalid() {
        assert!(Protocol::parse("thrift").is_err());
    }

    #[test]
    fn sampler_parse_valid() {
        assert_eq!(Sampler::parse("always_on").unwrap(), Sampler::AlwaysOn);
        assert_eq!(Sampler::parse("always_off").unwrap(), Sampler::AlwaysOff);
        assert_eq!(Sampler::parse("parent_based_traceid_ratio").unwrap(), Sampler::ParentBasedTraceIdRatio);
    }

    #[test]
    fn sampler_parse_invalid() {
        assert!(Sampler::parse("custom").is_err());
    }
}
