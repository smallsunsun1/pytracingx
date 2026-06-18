use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter, UpDownCounter};
use opentelemetry::{global, InstrumentationScope};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::error::PtxError;
use crate::traces::py_attributes_to_kv;

#[pyclass(module = "pytracingx._native", name = "Meter")]
pub struct PyMeter {
    inner: Meter,
}

impl PyMeter {
    pub fn new(name: String) -> Self {
        let scope = InstrumentationScope::builder(name).build();
        Self {
            inner: global::meter_with_scope(scope),
        }
    }
}

#[pymethods]
impl PyMeter {
    #[pyo3(signature = (name, *, unit = None, description = None))]
    fn counter(
        &self,
        name: String,
        unit: Option<String>,
        description: Option<String>,
    ) -> PyResult<PyCounter> {
        let mut builder = self.inner.f64_counter(name);
        if let Some(d) = description {
            builder = builder.with_description(d);
        }
        if let Some(u) = unit {
            builder = builder.with_unit(u);
        }
        Ok(PyCounter {
            inner: builder.build(),
        })
    }

    #[pyo3(signature = (name, *, unit = None, description = None))]
    fn up_down_counter(
        &self,
        name: String,
        unit: Option<String>,
        description: Option<String>,
    ) -> PyResult<PyUpDownCounter> {
        let mut builder = self.inner.f64_up_down_counter(name);
        if let Some(d) = description {
            builder = builder.with_description(d);
        }
        if let Some(u) = unit {
            builder = builder.with_unit(u);
        }
        Ok(PyUpDownCounter {
            inner: builder.build(),
        })
    }

    #[pyo3(signature = (name, *, unit = None, description = None, boundaries = None))]
    fn histogram(
        &self,
        name: String,
        unit: Option<String>,
        description: Option<String>,
        boundaries: Option<Vec<f64>>,
    ) -> PyResult<PyHistogram> {
        let mut builder = self.inner.f64_histogram(name);
        if let Some(d) = description {
            builder = builder.with_description(d);
        }
        if let Some(u) = unit {
            builder = builder.with_unit(u);
        }
        if let Some(b) = boundaries {
            builder = builder.with_boundaries(b);
        }
        Ok(PyHistogram {
            inner: builder.build(),
        })
    }

    #[pyo3(signature = (name, *, unit = None, description = None))]
    fn gauge(
        &self,
        name: String,
        unit: Option<String>,
        description: Option<String>,
    ) -> PyResult<PyGauge> {
        let mut builder = self.inner.f64_gauge(name);
        if let Some(d) = description {
            builder = builder.with_description(d);
        }
        if let Some(u) = unit {
            builder = builder.with_unit(u);
        }
        Ok(PyGauge {
            inner: builder.build(),
        })
    }

    fn __repr__(&self) -> &'static str {
        "Meter"
    }
}

#[pyclass(module = "pytracingx._native", name = "Counter")]
pub struct PyCounter {
    inner: Counter<f64>,
}

#[pymethods]
impl PyCounter {
    #[pyo3(signature = (value, attributes = None))]
    fn add(&self, value: f64, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let kvs = py_attributes_to_kv(attributes)?;
        self.inner.add(value, &kvs);
        Ok(())
    }
}

#[pyclass(module = "pytracingx._native", name = "UpDownCounter")]
pub struct PyUpDownCounter {
    inner: UpDownCounter<f64>,
}

#[pymethods]
impl PyUpDownCounter {
    #[pyo3(signature = (value, attributes = None))]
    fn add(&self, value: f64, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let kvs = py_attributes_to_kv(attributes)?;
        self.inner.add(value, &kvs);
        Ok(())
    }
}

#[pyclass(module = "pytracingx._native", name = "Histogram")]
pub struct PyHistogram {
    inner: Histogram<f64>,
}

#[pymethods]
impl PyHistogram {
    #[pyo3(signature = (value, attributes = None))]
    fn record(&self, value: f64, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let kvs = py_attributes_to_kv(attributes)?;
        self.inner.record(value, &kvs);
        Ok(())
    }
}

#[pyclass(module = "pytracingx._native", name = "Gauge")]
pub struct PyGauge {
    inner: Gauge<f64>,
}

#[pymethods]
impl PyGauge {
    #[pyo3(signature = (value, attributes = None))]
    fn record(&self, value: f64, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let kvs = py_attributes_to_kv(attributes)?;
        self.inner.record(value, &kvs);
        Ok(())
    }
}

#[pyfunction]
pub fn get_meter(name: String) -> PyResult<PyMeter> {
    if !crate::runtime::is_initialized() {
        return Err(PtxError::NotInitialized.into());
    }
    Ok(PyMeter::new(name))
}
