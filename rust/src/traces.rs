use opentelemetry::trace::{SpanContext, Status, TraceContextExt};
use opentelemetry::{global, KeyValue, StringValue, Value};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyString};
use tracing::field::Empty;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::context;
use crate::error::PtxError;

/// Start a new span rooted at the current contextvars context.
///
/// The underlying `tracing::Span` carries fixed metadata (target =
/// `pytracingx`, level = `INFO`); the dynamic `name` and `kind` arrive via the
/// `otel.name` / `otel.kind` field convention recognized by
/// `tracing-opentelemetry`.
#[pyfunction]
#[pyo3(signature = (name, attributes = None, kind = "internal".to_string()))]
pub fn start_span(
    py: Python<'_>,
    name: String,
    attributes: Option<&Bound<'_, PyDict>>,
    kind: String,
) -> PyResult<PySpan> {
    if !crate::runtime::is_initialized() {
        return Err(PtxError::NotInitialized.into());
    }
    validate_span_kind(&kind)?;

    let span = tracing::info_span!(
        target: "pytracingx",
        "py_span",
        otel.name = Empty,
        otel.kind = Empty,
    );
    span.record("otel.name", tracing::field::display(&name));
    span.record("otel.kind", tracing::field::display(&kind));

    // Re-parent if Python contextvars hold a remote/parent OTel context
    // (e.g. propagated from incoming HTTP headers).
    let parent_ctx = context::current(py)?;
    if parent_ctx.span().span_context().is_valid() {
        let _ = span.set_parent(parent_ctx);
    }

    if let Some(attrs) = attributes {
        for (k, v) in attrs.iter() {
            let key = k.extract::<String>()?;
            let value = py_to_otel_value(&v)?;
            span.set_attribute(key, value);
        }
    }

    let new_ctx = span.context();
    let token = context::push(py, new_ctx)?;
    Ok(PySpan {
        span: Some(span),
        token: Some(token),
        ended: false,
    })
}

/// A live span. Owns the underlying `tracing::Span` and the contextvars
/// token that we placed when the span was started.
#[pyclass(module = "pytracingx._native", name = "Span", unsendable)]
pub struct PySpan {
    span: Option<Span>,
    token: Option<Py<PyAny>>,
    ended: bool,
}

#[pymethods]
impl PySpan {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<&Bound<'_, PyAny>>,
        exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        if let (Some(_), Some(val)) = (exc_type, exc_val) {
            let msg = val
                .str()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "exception".to_string());
            if let Some(span) = &self.span {
                span.set_status(Status::error(msg.clone()));
                span.add_event(
                    "exception",
                    vec![KeyValue::new("exception.message", msg)],
                );
            }
        }
        self.end_internal(py)
    }

    fn set_attribute(&mut self, key: String, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(span) = &self.span {
            let v = py_to_otel_value(value)?;
            span.set_attribute(key, v);
        }
        Ok(())
    }

    #[pyo3(signature = (code, description = None))]
    fn set_status(&mut self, code: String, description: Option<String>) -> PyResult<()> {
        let status = match code.as_str() {
            "ok" => Status::Ok,
            "unset" => Status::Unset,
            "error" => Status::error(description.unwrap_or_default()),
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown status code '{other}', expected 'ok' | 'error' | 'unset'"
                )))
            }
        };
        if let Some(span) = &self.span {
            span.set_status(status);
        }
        Ok(())
    }

    fn update_name(&mut self, name: String) {
        if let Some(span) = &self.span {
            span.record("otel.name", tracing::field::display(&name));
        }
    }

    #[pyo3(signature = (name, attributes = None))]
    fn add_event(&mut self, name: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let kvs = py_attributes_to_kv(attributes)?;
        if let Some(span) = &self.span {
            span.add_event(name, kvs);
        }
        Ok(())
    }

    #[pyo3(signature = (exception, escaped = false))]
    fn record_exception(&mut self, exception: &Bound<'_, PyAny>, escaped: bool) -> PyResult<()> {
        let kind = exception
            .get_type()
            .name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "Exception".to_string());
        let message = exception
            .str()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let attrs = vec![
            KeyValue::new("exception.type", kind),
            KeyValue::new("exception.message", message.clone()),
            KeyValue::new("exception.escaped", escaped),
        ];
        if let Some(span) = &self.span {
            span.add_event("exception", attrs);
            span.set_status(Status::error(message));
        }
        Ok(())
    }

    fn end(&mut self, py: Python<'_>) -> PyResult<()> {
        self.end_internal(py)?;
        Ok(())
    }

    fn trace_id(&self) -> Option<String> {
        self.span_context().filter(|sc| sc.is_valid()).map(|sc| {
            format!("{:032x}", u128::from_be_bytes(sc.trace_id().to_bytes()))
        })
    }

    fn span_id(&self) -> Option<String> {
        self.span_context().filter(|sc| sc.is_valid()).map(|sc| {
            format!("{:016x}", u64::from_be_bytes(sc.span_id().to_bytes()))
        })
    }

    fn is_recording(&self) -> bool {
        self.span_context()
            .map(|sc| sc.is_valid())
            .unwrap_or(false)
    }
}

impl PySpan {
    fn span_context(&self) -> Option<SpanContext> {
        self.span
            .as_ref()
            .map(|s| s.context().span().span_context().clone())
    }

    fn end_internal(&mut self, py: Python<'_>) -> PyResult<bool> {
        if self.ended {
            return Ok(false);
        }
        self.ended = true;
        // Dropping the tracing::Span ends the underlying OTel span via the
        // `tracing-opentelemetry` layer's on_close hook.
        self.span = None;
        if let Some(token) = self.token.take() {
            context::restore(py, token)?;
        }
        Ok(false)
    }
}

impl Drop for PySpan {
    fn drop(&mut self) {
        if !self.ended {
            self.span = None;
        }
    }
}

fn validate_span_kind(value: &str) -> PyResult<()> {
    match value {
        "internal" | "server" | "client" | "producer" | "consumer" => Ok(()),
        other => Err(PyValueError::new_err(format!("unknown span kind '{other}'"))),
    }
}

pub fn py_attributes_to_kv(attrs: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<KeyValue>> {
    let Some(dict) = attrs else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let k = key.extract::<String>()?;
        let v = py_to_otel_value(&value)?;
        out.push(KeyValue::new(k, v));
    }
    Ok(out)
}

fn py_to_otel_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if value.is_instance_of::<PyBool>() {
        return Ok(Value::Bool(value.extract::<bool>()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(Value::I64(value.extract::<i64>()?));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(Value::F64(value.extract::<f64>()?));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(Value::String(StringValue::from(value.extract::<String>()?)));
    }
    let repr = value.str()?.to_string_lossy().into_owned();
    Ok(Value::String(StringValue::from(repr)))
}

pub fn current_trace_and_span_id(py: Python<'_>) -> PyResult<(Option<String>, Option<String>)> {
    let ctx = context::current(py)?;
    let span = ctx.span();
    let sc = span.span_context();
    if !sc.is_valid() {
        return Ok((None, None));
    }
    let trace = format!("{:032x}", u128::from_be_bytes(sc.trace_id().to_bytes()));
    let span_id = format!("{:016x}", u64::from_be_bytes(sc.span_id().to_bytes()));
    Ok((Some(trace), Some(span_id)))
}

#[pyfunction]
pub fn inject_headers(py: Python<'_>, carrier: &Bound<'_, PyDict>) -> PyResult<()> {
    use opentelemetry::propagation::Injector;
    struct DictInjector<'a, 'py>(&'a Bound<'py, PyDict>);
    impl<'a, 'py> Injector for DictInjector<'a, 'py> {
        fn set(&mut self, key: &str, value: String) {
            let _ = self.0.set_item(key, value);
        }
    }
    let ctx = context::current(py)?;
    let mut injector = DictInjector(carrier);
    global::get_text_map_propagator(|prop| {
        prop.inject_context(&ctx, &mut injector);
    });
    Ok(())
}

#[pyfunction]
pub fn extract_headers(py: Python<'_>, carrier: &Bound<'_, PyDict>) -> PyResult<ExtractedContext> {
    use opentelemetry::propagation::Extractor;

    struct SmallCarrier {
        traceparent: Option<String>,
        tracestate: Option<String>,
    }
    impl Extractor for SmallCarrier {
        fn get(&self, key: &str) -> Option<&str> {
            match key {
                "traceparent" => self.traceparent.as_deref(),
                "tracestate" => self.tracestate.as_deref(),
                _ => None,
            }
        }
        fn keys(&self) -> Vec<&str> {
            let mut keys = Vec::with_capacity(2);
            if self.traceparent.is_some() {
                keys.push("traceparent");
            }
            if self.tracestate.is_some() {
                keys.push("tracestate");
            }
            keys
        }
    }

    let traceparent = carrier
        .get_item("traceparent")?
        .map(|v| v.extract::<String>())
        .transpose()?;
    let tracestate = carrier
        .get_item("tracestate")?
        .map(|v| v.extract::<String>())
        .transpose()?;

    let extractor = SmallCarrier { traceparent, tracestate };
    let parent = global::get_text_map_propagator(|prop| prop.extract(&extractor));
    let token = context::push(py, parent)?;
    Ok(ExtractedContext { token: Some(token) })
}

/// Holds the contextvars token from `extract_headers`. Supports `with` syntax
/// to auto-restore the previous context on exit.
///
/// ```python
/// with ptx.extract_headers(request.headers):
///     with ptx.start_span("handle", kind="server"):
///         ...
/// # context is automatically restored here
/// ```
#[pyclass(module = "pytracingx._native", name = "ExtractedContext", unsendable)]
pub struct ExtractedContext {
    token: Option<Py<PyAny>>,
}

#[pymethods]
impl ExtractedContext {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        if let Some(token) = self.token.take() {
            context::restore(py, token)?;
        }
        Ok(false)
    }

    fn restore(&mut self, py: Python<'_>) -> PyResult<()> {
        if let Some(token) = self.token.take() {
            context::restore(py, token)?;
        }
        Ok(())
    }
}

impl Drop for ExtractedContext {
    fn drop(&mut self) {
        // If the user forgot to restore, the contextvars copy-on-write
        // semantics handle cleanup. We intentionally don't acquire the GIL
        // here to avoid deadlocks on interpreter shutdown.
    }
}

#[pyfunction]
pub fn restore_context(py: Python<'_>, token: Py<PyAny>) -> PyResult<()> {
    context::restore(py, token)
}
