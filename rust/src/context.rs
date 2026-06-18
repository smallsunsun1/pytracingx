use opentelemetry::Context;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;

/// Shared `contextvars.ContextVar` storing the current OTel context.
static CONTEXT_VAR: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
const CV_NAME: &str = "pytracingx_otel_ctx";

fn context_var<'py>(py: Python<'py>) -> PyResult<&'py Py<PyAny>> {
    CONTEXT_VAR.get_or_try_init(py, || {
        let contextvars = py.import("contextvars")?;
        let kwargs = pyo3::types::PyDict::new(py);
        kwargs.set_item("default", py.None())?;
        let cv = contextvars
            .getattr("ContextVar")?
            .call((CV_NAME,), Some(&kwargs))?;
        Ok::<Py<PyAny>, PyErr>(cv.unbind())
    })
}

/// Snapshot the OTel `Context` currently stored in the Python contextvars.
/// Falls back to `Context::current()` when nothing has been pushed yet.
pub fn current(py: Python<'_>) -> PyResult<Context> {
    let cv = context_var(py)?;
    let stored = cv.bind(py).call_method0("get")?;
    if stored.is_none() {
        return Ok(Context::current());
    }
    let holder: PyRef<'_, ContextHolder> = stored.extract()?;
    Ok(holder.ctx.clone())
}

/// Push the supplied `Context` onto the Python contextvars and return the
/// token that callers use to restore the prior value.
pub fn push(py: Python<'_>, ctx: Context) -> PyResult<Py<PyAny>> {
    let cv = context_var(py)?;
    let holder = Bound::new(py, ContextHolder { ctx })?;
    let token = cv.bind(py).call_method1("set", (holder,))?;
    Ok(token.unbind())
}

pub fn restore(py: Python<'_>, token: Py<PyAny>) -> PyResult<()> {
    let cv = context_var(py)?;
    cv.bind(py).call_method1("reset", (token,))?;
    Ok(())
}

/// Opaque holder so we can stash an `opentelemetry::Context` inside a Python
/// `ContextVar`. The Context is `Clone + Send + Sync`, so it's safe to keep
/// inside an immutable pyclass.
#[pyclass(module = "pytracingx._native", name = "_ContextHolder", frozen)]
pub struct ContextHolder {
    pub ctx: Context,
}

#[pymethods]
impl ContextHolder {
    fn __repr__(&self) -> &'static str {
        "<pytracingx _ContextHolder>"
    }
}
