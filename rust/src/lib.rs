use pyo3::prelude::*;
use pyo3::types::PyDict;

mod config;
mod context;
mod error;
mod exporters;
mod logs;
mod metrics;
mod runtime;
mod sls;
mod sls_log;
mod traces;

/// Initialize the global pytracingx providers from a `Config` instance.
///
/// Idempotent: calling `init` while already initialized raises
/// `RuntimeError`. Use `shutdown()` first if you need to swap configs.
#[pyfunction]
fn init(py: Python<'_>, config: &config::PyConfig) -> PyResult<()> {
    let resolved = config.resolve()?;
    py.detach(|| runtime::install(resolved))?;
    Ok(())
}

/// Gracefully flush pending data and tear down all providers.
#[pyfunction]
fn shutdown(py: Python<'_>) -> PyResult<()> {
    py.detach(runtime::uninstall)?;
    Ok(())
}

/// Force-flush traces, metrics and logs synchronously without tearing down.
#[pyfunction]
fn force_flush(py: Python<'_>) -> PyResult<()> {
    py.detach(runtime::force_flush)?;
    Ok(())
}

/// Returns the current trace_id and span_id (hex) from the active contextvars.
#[pyfunction]
fn current_trace_context<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let (trace_id, span_id) = traces::current_trace_and_span_id(py)?;
    let dict = PyDict::new(py);
    dict.set_item("trace_id", trace_id)?;
    dict.set_item("span_id", span_id)?;
    Ok(dict)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    runtime::init_default();

    // configuration
    m.add_class::<config::PyConfig>()?;
    m.add_class::<config::PyTraceSink>()?;
    m.add_class::<config::PyMetricSink>()?;
    m.add_class::<config::PyOtlpLogSink>()?;
    m.add_class::<config::PySlsLogSink>()?;
    m.add_class::<config::PyRawOtlp>()?;
    m.add_class::<context::ContextHolder>()?;

    // tracing
    m.add_class::<traces::PySpan>()?;
    m.add_class::<traces::ExtractedContext>()?;
    m.add_function(wrap_pyfunction!(traces::start_span, m)?)?;
    m.add_function(wrap_pyfunction!(traces::inject_headers, m)?)?;
    m.add_function(wrap_pyfunction!(traces::extract_headers, m)?)?;
    m.add_function(wrap_pyfunction!(traces::restore_context, m)?)?;

    // metrics
    m.add_class::<metrics::PyMeter>()?;
    m.add_class::<metrics::PyCounter>()?;
    m.add_class::<metrics::PyUpDownCounter>()?;
    m.add_class::<metrics::PyHistogram>()?;
    m.add_class::<metrics::PyGauge>()?;
    m.add_function(wrap_pyfunction!(metrics::get_meter, m)?)?;

    // logs
    m.add_class::<logs::PyLogger>()?;
    m.add_function(wrap_pyfunction!(logs::get_logger, m)?)?;
    m.add_function(wrap_pyfunction!(logs::emit_log_record, m)?)?;

    // lifecycle
    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;
    m.add_function(wrap_pyfunction!(force_flush, m)?)?;
    m.add_function(wrap_pyfunction!(current_trace_context, m)?)?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
