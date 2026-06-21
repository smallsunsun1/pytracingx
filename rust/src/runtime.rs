use std::any::TypeId;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLoggerProvider};
use opentelemetry_sdk::metrics::{
    SdkMeterProvider, periodic_reader_with_async_runtime::PeriodicReader,
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, Sampler as SdkSampler, SdkTracerProvider, SpanLimits,
    span_processor_with_async_runtime::BatchSpanProcessor,
};
use parking_lot::Mutex;
use tracing::span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::Registry;

use crate::config::{ResolvedConfig, Sampler};
use crate::error::{Result, anyhow};
use crate::exporters::{build_log_exporter, build_metric_exporter, build_span_exporter};
use crate::sls::build_resource;
use crate::sls_log::{self, SlsLogHandle};

type BoxedLayer = Box<dyn tracing_subscriber::Layer<Registry> + Send + Sync>;

// ---------------------------------------------------------------------------
// SwappableLayers: hot-swap container for N layers behind RwLock.
// Properly forwards downcast_raw (reload::Layer blocks it).
// Arc<Self> serves as the handle — call .replace() to hot-swap.
// ---------------------------------------------------------------------------

struct SwappableLayers {
    inner: RwLock<Vec<BoxedLayer>>,
    filter: EnvFilter,
}

impl SwappableLayers {
    fn new(filter: EnvFilter) -> Self {
        Self {
            inner: RwLock::new(Vec::new()),
            filter,
        }
    }

    fn replace(&self, layers: Vec<BoxedLayer>) {
        *self.inner.write().unwrap() = layers;
    }
}

impl tracing_subscriber::Layer<Registry> for SwappableLayers {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if !self.filter.enabled(attrs.metadata(), ctx.clone()) { return; }
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_new_span(attrs, id, ctx.clone()); } }
    }
    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_record(id, values, ctx.clone()); } }
    }
    fn on_follows_from(&self, id: &span::Id, follows: &span::Id, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_follows_from(id, follows, ctx.clone()); } }
    }
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if !self.filter.enabled(event.metadata(), ctx.clone()) { return; }
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_event(event, ctx.clone()); } }
    }
    fn on_enter(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_enter(id, ctx.clone()); } }
    }
    fn on_exit(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_exit(id, ctx.clone()); } }
    }
    fn on_close(&self, id: span::Id, ctx: tracing_subscriber::layer::Context<'_, Registry>) {
        if let Ok(g) = self.inner.read() { for l in g.iter() { l.on_close(id.clone(), ctx.clone()); } }
    }
    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        if let Ok(g) = self.inner.read() {
            for l in g.iter() {
                let r = unsafe { l.downcast_raw(id) };
                if r.is_some() { return r; }
            }
        }
        (id == TypeId::of::<Self>()).then(|| self as *const _ as *const ())
    }
}

/// Newtype so Arc<SwappableLayers> can be boxed into the subscriber's Vec.
struct SwappableSlot(Arc<SwappableLayers>);

impl tracing_subscriber::Layer<Registry> for SwappableSlot {
    fn on_new_span(&self, a: &span::Attributes<'_>, id: &span::Id, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_new_span(a, id, c); }
    fn on_record(&self, id: &span::Id, v: &span::Record<'_>, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_record(id, v, c); }
    fn on_follows_from(&self, id: &span::Id, f: &span::Id, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_follows_from(id, f, c); }
    fn on_event(&self, e: &tracing::Event<'_>, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_event(e, c); }
    fn on_enter(&self, id: &span::Id, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_enter(id, c); }
    fn on_exit(&self, id: &span::Id, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_exit(id, c); }
    fn on_close(&self, id: span::Id, c: tracing_subscriber::layer::Context<'_, Registry>) { self.0.on_close(id, c); }
    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> { unsafe { self.0.downcast_raw(id) } }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

struct Global {
    logs: Arc<SwappableLayers>,
    metrics: Arc<SwappableLayers>,
    traces: Arc<SwappableLayers>,
    state: Mutex<Option<RuntimeState>>,
}

pub struct RuntimeState {
    pub tracer_providers: Vec<SdkTracerProvider>,
    pub meter_providers: Vec<SdkMeterProvider>,
    pub logger_providers: Vec<SdkLoggerProvider>,
    pub sls_log_handles: Vec<SlsLogHandle>,
}

static GLOBAL: OnceLock<Global> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn init_default() {
    let make_filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let logs = Arc::new(SwappableLayers::new(make_filter()));
    let metrics = Arc::new(SwappableLayers::new(make_filter()));
    let traces = Arc::new(SwappableLayers::new(make_filter()));

    logs.replace(vec![default_fmt_layer()]);

    let layers: Vec<BoxedLayer> = vec![
        Box::new(SwappableSlot(Arc::clone(&logs))),
        Box::new(SwappableSlot(Arc::clone(&metrics))),
        Box::new(SwappableSlot(Arc::clone(&traces))),
    ];

    tracing_subscriber::registry().with(layers).init();

    let _ = GLOBAL.set(Global {
        logs,
        metrics,
        traces,
        state: Mutex::new(None),
    });
}

pub fn install(config: ResolvedConfig) -> Result<()> {
    let g = GLOBAL.get().ok_or_else(|| anyhow!("runtime not initialized"))?;
    let mut guard = g.state.lock();
    if guard.is_some() {
        return Err(anyhow!(
            "pytracingx is already initialized; call pytracingx.shutdown() before re-initializing"
        ));
    }

    let resource = build_resource(&config);
    let runtime = pyo3_async_runtimes::tokio::get_runtime();

    let (tracer_providers, meter_providers, logger_providers) = runtime.block_on(async {
        let mut tracer_providers = Vec::new();
        for t in &config.traces {
            let signal = crate::config::SignalTransport {
                endpoint: &t.endpoint, protocol: t.protocol, headers: &t.headers,
                timeout_ms: t.timeout_ms, raw_otlp: &t.raw_otlp, temporality: None,
            };
            let exporter = build_span_exporter(&signal)?;
            let mut bb = BatchConfigBuilder::default();
            if let Some(v) = t.batch_max_queue { bb = bb.with_max_queue_size(v); }
            if let Some(v) = t.batch_max_export { bb = bb.with_max_export_batch_size(v); }
            if let Some(v) = t.batch_schedule_delay_ms { bb = bb.with_scheduled_delay(Duration::from_millis(v)); }
            if let Some(v) = t.max_export_timeout_ms { bb = bb.with_max_export_timeout(Duration::from_millis(v)); }
            let processor = BatchSpanProcessor::builder(exporter, Tokio).with_batch_config(bb.build()).build();
            let mut limits = SpanLimits::default();
            if let Some(v) = t.max_attributes_per_span { limits.max_attributes_per_span = v; }
            if let Some(v) = t.max_events_per_span { limits.max_events_per_span = v; }
            if let Some(v) = t.max_links_per_span { limits.max_links_per_span = v; }
            if let Some(v) = t.max_attributes_per_event { limits.max_attributes_per_event = v; }
            if let Some(v) = t.max_attributes_per_link { limits.max_attributes_per_link = v; }
            let provider = SdkTracerProvider::builder()
                .with_span_processor(processor)
                .with_resource(resource.clone())
                .with_sampler(map_sampler(t.sampler, t.sampler_arg))
                .with_span_limits(limits)
                .build();
            tracer_providers.push(provider);
        }
        if let Some(first) = tracer_providers.first() {
            global::set_tracer_provider(first.clone());
        }

        let mut meter_providers = Vec::new();
        for m in &config.metrics {
            let signal = crate::config::SignalTransport {
                endpoint: &m.endpoint, protocol: m.protocol, headers: &m.headers,
                timeout_ms: m.timeout_ms, raw_otlp: &m.raw_otlp, temporality: m.temporality,
            };
            let exporter = build_metric_exporter(&signal)?;
            let mut rb = PeriodicReader::builder(exporter, Tokio);
            if let Some(v) = m.export_interval_ms { rb = rb.with_interval(Duration::from_millis(v)); }
            if let Some(v) = m.export_timeout_ms { rb = rb.with_timeout(Duration::from_millis(v)); }
            let provider = SdkMeterProvider::builder()
                .with_reader(rb.build())
                .with_resource(resource.clone())
                .build();
            meter_providers.push(provider);
        }
        if let Some(first) = meter_providers.first() {
            global::set_meter_provider(first.clone());
        }

        let mut logger_providers = Vec::new();
        for l in &config.otlp_logs {
            let signal = crate::config::SignalTransport {
                endpoint: &l.endpoint, protocol: l.protocol, headers: &l.headers,
                timeout_ms: l.timeout_ms, raw_otlp: &l.raw_otlp, temporality: None,
            };
            let exporter = build_log_exporter(&signal)?;
            let processor = BatchLogProcessor::builder(exporter).build();
            let provider = SdkLoggerProvider::builder()
                .with_log_processor(processor)
                .with_resource(resource.clone())
                .build();
            logger_providers.push(provider);
        }

        Ok::<_, anyhow::Error>((tracer_providers, meter_providers, logger_providers))
    })?;

    // --- Replace metrics layers ---
    {
        use tracing_subscriber::Layer;
        let mut metric_layers: Vec<BoxedLayer> = Vec::new();
        for mp in &meter_providers {
            metric_layers.push(tracing_opentelemetry::MetricsLayer::new(mp.clone()).boxed());
        }
        g.metrics.replace(metric_layers);
    }

    // --- Replace trace layers ---
    {
        use tracing_subscriber::Layer;
        let mut trace_layers: Vec<BoxedLayer> = Vec::new();
        for tp in &tracer_providers {
            trace_layers.push(
                tracing_opentelemetry::layer()
                    .with_tracer(tp.tracer("pytracingx"))
                    .with_tracked_inactivity(false)
                    .with_threads(false)
                    .boxed(),
            );
        }
        g.traces.replace(trace_layers);
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    let mut sls_log_handles = Vec::new();
    {
        let _guard = runtime.enter();
        // --- Replace log layers (needs tokio context for SLS) ---
        {
            use tracing_subscriber::Layer;
            let mut log_layers: Vec<BoxedLayer> = Vec::new();
            if config.console_output {
                log_layers.push(build_fmt_layer(&config.console_format));
            }
            for lp in &logger_providers {
                log_layers.push(OpenTelemetryTracingBridge::new(lp).boxed());
            }
            for sls_cfg in &config.sls_logs {
                let (sls_layer, handle) =
                    sls_log::create_sls_log_layer(sls_cfg, &config.service_name)?;
                log_layers.push(sls_layer.boxed());
                sls_log_handles.push(handle);
            }
            g.logs.replace(log_layers);
        }
    }

    *guard = Some(RuntimeState {
        tracer_providers,
        meter_providers,
        logger_providers,
        sls_log_handles,
    });
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let g = GLOBAL.get().ok_or_else(|| anyhow!("runtime not initialized"))?;
    let mut guard = g.state.lock();
    let Some(state) = guard.take() else { return Ok(()); };

    for handle in &state.sls_log_handles {
        handle.flush_and_shutdown();
    }

    let runtime = pyo3_async_runtimes::tokio::get_runtime();
    runtime.block_on(async { tokio::time::sleep(Duration::from_millis(200)).await });

    for tp in &state.tracer_providers { let _ = tp.shutdown(); }
    for mp in &state.meter_providers { let _ = mp.shutdown(); }
    for lp in &state.logger_providers { let _ = lp.shutdown(); }

    g.logs.replace(vec![default_fmt_layer()]);
    g.metrics.replace(Vec::new());
    g.traces.replace(Vec::new());
    Ok(())
}

pub fn force_flush() -> Result<()> {
    let g = GLOBAL.get().ok_or_else(|| anyhow!("runtime not initialized"))?;
    let guard = g.state.lock();
    let Some(state) = guard.as_ref() else { return Ok(()); };
    for tp in &state.tracer_providers {
        if let Err(e) = tp.force_flush() { tracing::warn!("trace force_flush error: {e:?}"); }
    }
    for mp in &state.meter_providers {
        if let Err(e) = mp.force_flush() { tracing::warn!("metric force_flush error: {e:?}"); }
    }
    for lp in &state.logger_providers {
        if let Err(e) = lp.force_flush() { tracing::warn!("log force_flush error: {e:?}"); }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_fmt_layer() -> BoxedLayer {
    use tracing_subscriber::Layer;
    tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(std::io::stderr)
        .boxed()
}

fn build_fmt_layer(format: &str) -> BoxedLayer {
    use tracing_subscriber::Layer;
    use tracing_subscriber::fmt;
    match format {
        "json" => fmt::layer().json().with_target(true).with_writer(std::io::stderr).boxed(),
        "pretty" => fmt::layer().pretty().with_writer(std::io::stderr).boxed(),
        _ => fmt::layer().compact().with_writer(std::io::stderr).boxed(),
    }
}

fn map_sampler(s: Sampler, ratio: f64) -> SdkSampler {
    match s {
        Sampler::AlwaysOn => SdkSampler::AlwaysOn,
        Sampler::AlwaysOff => SdkSampler::AlwaysOff,
        Sampler::ParentBasedTraceIdRatio => SdkSampler::ParentBased(Box::new(
            SdkSampler::TraceIdRatioBased(ratio.clamp(0.0, 1.0)),
        )),
    }
}
