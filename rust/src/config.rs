use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::error::{Result, bail};

// ─── Protocol / Sampler / Temporality / Compression enums ────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Grpc,
    HttpProtobuf,
}

impl Protocol {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "grpc" => Ok(Protocol::Grpc),
            "http/protobuf" | "http" => Ok(Protocol::HttpProtobuf),
            other => bail!(format!(
                "unknown protocol '{other}', expected 'grpc' or 'http/protobuf'"
            )),
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
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "always_on" => Ok(Sampler::AlwaysOn),
            "always_off" => Ok(Sampler::AlwaysOff),
            "parent_based_traceid_ratio" => Ok(Sampler::ParentBasedTraceIdRatio),
            other => bail!(format!(
                "unknown sampler '{other}', expected 'always_on' | 'always_off' | 'parent_based_traceid_ratio'"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Temporality {
    Cumulative,
    Delta,
    LowMemory,
}

impl Temporality {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "cumulative" => Ok(Temporality::Cumulative),
            "delta" => Ok(Temporality::Delta),
            "lowmemory" | "low_memory" => Ok(Temporality::LowMemory),
            other => bail!(format!(
                "unknown temporality '{other}', expected 'cumulative' | 'delta' | 'lowmemory'"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Zstd,
}

impl Compression {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "gzip" => Ok(Compression::Gzip),
            "zstd" => Ok(Compression::Zstd),
            other => bail!(format!(
                "unknown compression '{other}', expected 'gzip' | 'zstd'"
            )),
        }
    }
}

// ─── Raw OTLP escape-hatch (typed pyclass) ───────────────────────────────────

/// Layer-3 escape hatch: a typed bag of low-level OTLP knobs.
///
/// Construct with named fields:
///
/// ```python
/// from pytracingx import RawOtlp
/// sink = ptx.TraceSink(endpoint="...", raw_otlp=RawOtlp(compression="gzip"))
/// ```
///
/// All fields are optional. Adding a new field here is a non-breaking change.
#[pyclass(module = "pytracingx._native", name = "RawOtlp", from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct PyRawOtlp {
    pub(crate) compression: Option<Compression>,
}

#[pymethods]
impl PyRawOtlp {
    #[new]
    #[pyo3(signature = (compression = None))]
    fn new(compression: Option<String>) -> anyhow::Result<Self> {
        let compression = compression
            .map(|s| Compression::parse(&s))
            .transpose()?;
        Ok(Self { compression })
    }

    fn __repr__(&self) -> String {
        match self.compression {
            Some(Compression::Gzip) => "RawOtlp(compression='gzip')".to_string(),
            Some(Compression::Zstd) => "RawOtlp(compression='zstd')".to_string(),
            None => "RawOtlp()".to_string(),
        }
    }
}

// ─── Sink pyclasses ──────────────────────────────────────────────────────────
//
// All numeric / string knobs default to None; when the runtime builds the
// underlying OTel SDK it only calls the corresponding `with_*` setter when
// the user supplied a value, otherwise the OTel-rust SDK default applies.

#[pyclass(module = "pytracingx._native", name = "TraceSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTraceSink {
    pub(crate) endpoint: String,
    pub(crate) protocol: Option<String>,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) sampler: Option<String>,
    pub(crate) sampler_arg: Option<f64>,
    pub(crate) batch_max_queue: Option<usize>,
    pub(crate) batch_max_export: Option<usize>,
    pub(crate) batch_schedule_delay_ms: Option<u64>,
    pub(crate) max_export_timeout_ms: Option<u64>,
    pub(crate) max_attributes_per_span: Option<u32>,
    pub(crate) max_events_per_span: Option<u32>,
    pub(crate) max_links_per_span: Option<u32>,
    pub(crate) max_attributes_per_event: Option<u32>,
    pub(crate) max_attributes_per_link: Option<u32>,
    pub(crate) raw_otlp: PyRawOtlp,
}

#[pymethods]
impl PyTraceSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        protocol = None,
        headers = None,
        timeout_ms = None,
        sampler = None,
        sampler_arg = None,
        batch_max_queue = None,
        batch_max_export = None,
        batch_schedule_delay_ms = None,
        max_export_timeout_ms = None,
        max_attributes_per_span = None,
        max_events_per_span = None,
        max_links_per_span = None,
        max_attributes_per_event = None,
        max_attributes_per_link = None,
        raw_otlp = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        endpoint: String,
        protocol: Option<String>,
        headers: Option<HashMap<String, String>>,
        timeout_ms: Option<u64>,
        sampler: Option<String>,
        sampler_arg: Option<f64>,
        batch_max_queue: Option<usize>,
        batch_max_export: Option<usize>,
        batch_schedule_delay_ms: Option<u64>,
        max_export_timeout_ms: Option<u64>,
        max_attributes_per_span: Option<u32>,
        max_events_per_span: Option<u32>,
        max_links_per_span: Option<u32>,
        max_attributes_per_event: Option<u32>,
        max_attributes_per_link: Option<u32>,
        raw_otlp: Option<PyRawOtlp>,
    ) -> anyhow::Result<Self> {
        if endpoint.is_empty() {
            bail!("TraceSink endpoint must not be empty");
        }
        if let Some(p) = &protocol {
            Protocol::parse(p)?;
        }
        if let Some(s) = &sampler {
            Sampler::parse(s)?;
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
            max_export_timeout_ms,
            max_attributes_per_span,
            max_events_per_span,
            max_links_per_span,
            max_attributes_per_event,
            max_attributes_per_link,
            raw_otlp: raw_otlp.unwrap_or_default(),
        })
    }
}

#[pyclass(module = "pytracingx._native", name = "MetricSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMetricSink {
    pub(crate) endpoint: String,
    pub(crate) protocol: Option<String>,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) export_interval_ms: Option<u64>,
    pub(crate) export_timeout_ms: Option<u64>,
    pub(crate) temporality: Option<String>,
    pub(crate) raw_otlp: PyRawOtlp,
}

#[pymethods]
impl PyMetricSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        protocol = None,
        headers = None,
        timeout_ms = None,
        export_interval_ms = None,
        export_timeout_ms = None,
        temporality = None,
        raw_otlp = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        endpoint: String,
        protocol: Option<String>,
        headers: Option<HashMap<String, String>>,
        timeout_ms: Option<u64>,
        export_interval_ms: Option<u64>,
        export_timeout_ms: Option<u64>,
        temporality: Option<String>,
        raw_otlp: Option<PyRawOtlp>,
    ) -> anyhow::Result<Self> {
        if endpoint.is_empty() {
            bail!("MetricSink endpoint must not be empty");
        }
        if let Some(p) = &protocol {
            Protocol::parse(p)?;
        }
        if let Some(t) = &temporality {
            Temporality::parse(t)?;
        }
        Ok(Self {
            endpoint,
            protocol,
            headers: headers.unwrap_or_default(),
            timeout_ms,
            export_interval_ms,
            export_timeout_ms,
            temporality,
            raw_otlp: raw_otlp.unwrap_or_default(),
        })
    }
}

#[pyclass(module = "pytracingx._native", name = "OtlpLogSink", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyOtlpLogSink {
    pub(crate) endpoint: String,
    pub(crate) protocol: Option<String>,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) batch_max_queue: Option<usize>,
    pub(crate) batch_max_export: Option<usize>,
    pub(crate) batch_schedule_delay_ms: Option<u64>,
    pub(crate) max_export_timeout_ms: Option<u64>,
    pub(crate) raw_otlp: PyRawOtlp,
}

#[pymethods]
impl PyOtlpLogSink {
    #[new]
    #[pyo3(signature = (
        endpoint,
        protocol = None,
        headers = None,
        timeout_ms = None,
        batch_max_queue = None,
        batch_max_export = None,
        batch_schedule_delay_ms = None,
        max_export_timeout_ms = None,
        raw_otlp = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        endpoint: String,
        protocol: Option<String>,
        headers: Option<HashMap<String, String>>,
        timeout_ms: Option<u64>,
        batch_max_queue: Option<usize>,
        batch_max_export: Option<usize>,
        batch_schedule_delay_ms: Option<u64>,
        max_export_timeout_ms: Option<u64>,
        raw_otlp: Option<PyRawOtlp>,
    ) -> anyhow::Result<Self> {
        if endpoint.is_empty() {
            bail!("OtlpLogSink endpoint must not be empty");
        }
        if let Some(p) = &protocol {
            Protocol::parse(p)?;
        }
        Ok(Self {
            endpoint,
            protocol,
            headers: headers.unwrap_or_default(),
            timeout_ms,
            batch_max_queue,
            batch_max_export,
            batch_schedule_delay_ms,
            max_export_timeout_ms,
            raw_otlp: raw_otlp.unwrap_or_default(),
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
    ) -> anyhow::Result<Self> {
        if endpoint.is_empty() {
            bail!("SlsLogSink endpoint must not be empty");
        }
        if project.is_empty() {
            bail!("SlsLogSink project must not be empty");
        }
        if logstore.is_empty() {
            bail!("SlsLogSink logstore must not be empty");
        }
        if ak_id.is_empty() || ak_secret.is_empty() {
            bail!("SlsLogSink ak_id and ak_secret must not be empty");
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
//
// Resolved* mirrors Py* but with the protocol/sampler/temporality strings
// already parsed into typed enums. None values stay None — the runtime checks
// for Some(..) before invoking the SDK setter.

#[derive(Debug, Clone)]
pub struct ResolvedTraceSink {
    pub endpoint: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub sampler: Sampler,
    pub sampler_arg: f64,
    pub batch_max_queue: Option<usize>,
    pub batch_max_export: Option<usize>,
    pub batch_schedule_delay_ms: Option<u64>,
    pub max_export_timeout_ms: Option<u64>,
    pub max_attributes_per_span: Option<u32>,
    pub max_events_per_span: Option<u32>,
    pub max_links_per_span: Option<u32>,
    pub max_attributes_per_event: Option<u32>,
    pub max_attributes_per_link: Option<u32>,
    pub raw_otlp: PyRawOtlp,
}

#[derive(Debug, Clone)]
pub struct ResolvedMetricSink {
    pub endpoint: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub export_interval_ms: Option<u64>,
    pub export_timeout_ms: Option<u64>,
    pub temporality: Option<Temporality>,
    pub raw_otlp: PyRawOtlp,
}

#[derive(Debug, Clone)]
pub struct ResolvedOtlpLogSink {
    pub endpoint: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub batch_max_queue: Option<usize>,
    pub batch_max_export: Option<usize>,
    pub batch_schedule_delay_ms: Option<u64>,
    pub max_export_timeout_ms: Option<u64>,
    pub raw_otlp: PyRawOtlp,
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
    pub timeout_ms: Option<u64>,
    pub raw_otlp: &'a PyRawOtlp,
    pub temporality: Option<Temporality>,
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
    ) -> anyhow::Result<Self> {
        if service_name.is_empty() {
            bail!("service_name must not be empty");
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
                        bail!("only one TraceSink allowed");
                    }
                    trace_sink = Some(s);
                } else if let Ok(s) = obj.extract::<PyMetricSink>() {
                    if metric_sink.is_some() {
                        bail!("only one MetricSink allowed");
                    }
                    metric_sink = Some(s);
                } else if let Ok(s) = obj.extract::<PyOtlpLogSink>() {
                    if otlp_log_sink.is_some() {
                        bail!("only one OtlpLogSink allowed");
                    }
                    otlp_log_sink = Some(s);
                } else if let Ok(s) = obj.extract::<PySlsLogSink>() {
                    if sls_log_sink.is_some() {
                        bail!("only one SlsLogSink allowed");
                    }
                    sls_log_sink = Some(s);
                } else {
                    bail!(
                        "sinks list must contain TraceSink, MetricSink, OtlpLogSink, or SlsLogSink"
                    );
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

    fn describe<'py>(&self, py: Python<'py>) -> anyhow::Result<Bound<'py, PyDict>> {
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

const DEFAULT_PROTOCOL: &str = "grpc";
const DEFAULT_SAMPLER: &str = "parent_based_traceid_ratio";
const DEFAULT_SAMPLER_ARG: f64 = 1.0;

impl PyConfig {
    pub fn resolve(&self) -> Result<ResolvedConfig> {
        validate_console_format(&self.console_format)?;

        let traces = self
            .trace_sink
            .as_ref()
            .map(|s| -> Result<ResolvedTraceSink> {
                let protocol =
                    Protocol::parse(s.protocol.as_deref().unwrap_or(DEFAULT_PROTOCOL))?;
                let sampler = Sampler::parse(s.sampler.as_deref().unwrap_or(DEFAULT_SAMPLER))?;
                Ok(ResolvedTraceSink {
                    endpoint: s.endpoint.clone(),
                    protocol,
                    headers: s.headers.clone(),
                    timeout_ms: s.timeout_ms,
                    sampler,
                    sampler_arg: s.sampler_arg.unwrap_or(DEFAULT_SAMPLER_ARG),
                    batch_max_queue: s.batch_max_queue,
                    batch_max_export: s.batch_max_export,
                    batch_schedule_delay_ms: s.batch_schedule_delay_ms,
                    max_export_timeout_ms: s.max_export_timeout_ms,
                    max_attributes_per_span: s.max_attributes_per_span,
                    max_events_per_span: s.max_events_per_span,
                    max_links_per_span: s.max_links_per_span,
                    max_attributes_per_event: s.max_attributes_per_event,
                    max_attributes_per_link: s.max_attributes_per_link,
                    raw_otlp: s.raw_otlp.clone(),
                })
            })
            .transpose()?;

        let metrics = self
            .metric_sink
            .as_ref()
            .map(|s| -> Result<ResolvedMetricSink> {
                let protocol =
                    Protocol::parse(s.protocol.as_deref().unwrap_or(DEFAULT_PROTOCOL))?;
                let temporality = s
                    .temporality
                    .as_deref()
                    .map(Temporality::parse)
                    .transpose()?;
                Ok(ResolvedMetricSink {
                    endpoint: s.endpoint.clone(),
                    protocol,
                    headers: s.headers.clone(),
                    timeout_ms: s.timeout_ms,
                    export_interval_ms: s.export_interval_ms,
                    export_timeout_ms: s.export_timeout_ms,
                    temporality,
                    raw_otlp: s.raw_otlp.clone(),
                })
            })
            .transpose()?;

        let otlp_logs = self
            .otlp_log_sink
            .as_ref()
            .map(|s| -> Result<ResolvedOtlpLogSink> {
                let protocol =
                    Protocol::parse(s.protocol.as_deref().unwrap_or(DEFAULT_PROTOCOL))?;
                Ok(ResolvedOtlpLogSink {
                    endpoint: s.endpoint.clone(),
                    protocol,
                    headers: s.headers.clone(),
                    timeout_ms: s.timeout_ms,
                    batch_max_queue: s.batch_max_queue,
                    batch_max_export: s.batch_max_export,
                    batch_schedule_delay_ms: s.batch_schedule_delay_ms,
                    max_export_timeout_ms: s.max_export_timeout_ms,
                    raw_otlp: s.raw_otlp.clone(),
                })
            })
            .transpose()?;

        let sls_log = self.sls_log_sink.as_ref().map(|s| ResolvedSlsLogSink {
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

fn validate_console_format(fmt: &str) -> Result<()> {
    match fmt {
        "compact" | "pretty" | "json" => Ok(()),
        other => bail!(
            "unknown console_format '{other}', expected 'compact' | 'pretty' | 'json'"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_parse_valid() {
        assert_eq!(Protocol::parse("grpc").unwrap(), Protocol::Grpc);
        assert_eq!(
            Protocol::parse("http/protobuf").unwrap(),
            Protocol::HttpProtobuf
        );
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
        assert_eq!(
            Sampler::parse("parent_based_traceid_ratio").unwrap(),
            Sampler::ParentBasedTraceIdRatio
        );
    }

    #[test]
    fn sampler_parse_invalid() {
        assert!(Sampler::parse("custom").is_err());
    }

    #[test]
    fn temporality_parse_valid() {
        assert_eq!(Temporality::parse("cumulative").unwrap(), Temporality::Cumulative);
        assert_eq!(Temporality::parse("delta").unwrap(), Temporality::Delta);
        assert_eq!(Temporality::parse("LowMemory").unwrap(), Temporality::LowMemory);
    }

    #[test]
    fn temporality_parse_invalid() {
        assert!(Temporality::parse("foo").is_err());
    }

    #[test]
    fn compression_parse_valid() {
        assert_eq!(Compression::parse("gzip").unwrap(), Compression::Gzip);
        assert_eq!(Compression::parse("Zstd").unwrap(), Compression::Zstd);
    }

    #[test]
    fn compression_parse_invalid() {
        assert!(Compression::parse("snappy").is_err());
    }
}
