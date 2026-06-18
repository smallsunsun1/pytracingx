use std::time::Duration;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLoggerProvider};
use opentelemetry_sdk::metrics::{
    periodic_reader_with_async_runtime::PeriodicReader, SdkMeterProvider,
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::trace::{
    BatchConfigBuilder,
    span_processor_with_async_runtime::BatchSpanProcessor,
    Sampler as SdkSampler, SdkTracerProvider,
};
use parking_lot::Mutex;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

use crate::config::{ResolvedConfig, Sampler};
use crate::error::{anyhow, Result};
use crate::exporters::{build_log_exporter, build_metric_exporter, build_span_exporter};
use crate::sls::build_resource;
use crate::sls_log::{self, SlsLogHandle};

static STATE: once_cell::sync::OnceCell<Mutex<Option<RuntimeState>>> =
    once_cell::sync::OnceCell::new();

static GLOBAL_DISPATCH_INSTALLED: once_cell::sync::OnceCell<()> =
    once_cell::sync::OnceCell::new();

pub struct RuntimeState {
    pub tracer_provider: Option<SdkTracerProvider>,
    pub meter_provider: Option<SdkMeterProvider>,
    pub logger_provider: Option<SdkLoggerProvider>,
    pub sls_log_handle: Option<SlsLogHandle>,
}

fn cell() -> &'static Mutex<Option<RuntimeState>> {
    STATE.get_or_init(|| Mutex::new(None))
}

pub fn is_initialized() -> bool {
    cell().lock().is_some()
}

pub fn install(config: ResolvedConfig) -> Result<()> {
    let mut guard = cell().lock();
    if guard.is_some() {
        return Err(anyhow!("pytracingx is already initialized; call pytracingx.shutdown() before re-initializing"));
    }

    let resource = build_resource(&config);
    let runtime = pyo3_async_runtimes::tokio::get_runtime();

    let (tracer_provider, meter_provider, logger_provider) = runtime.block_on(async {
        let tracer_provider = if let Some(ref t) = config.traces {
            let signal = crate::config::SignalTransport {
                endpoint: &t.endpoint,
                protocol: t.protocol,
                headers: &t.headers,
                timeout_ms: t.timeout_ms,
            };
            let exporter = build_span_exporter(&signal)?;
            let batch_cfg = BatchConfigBuilder::default()
                .with_max_queue_size(t.batch_max_queue)
                .with_max_export_batch_size(t.batch_max_export)
                .with_scheduled_delay(Duration::from_millis(t.batch_schedule_delay_ms))
                .build();
            let processor = BatchSpanProcessor::builder(exporter, Tokio)
                .with_batch_config(batch_cfg)
                .build();
            let provider = SdkTracerProvider::builder()
                .with_span_processor(processor)
                .with_resource(resource.clone())
                .with_sampler(map_sampler(t.sampler, t.sampler_arg))
                .build();
            global::set_tracer_provider(provider.clone());
            Some(provider)
        } else {
            None
        };

        let meter_provider = if let Some(ref m) = config.metrics {
            let signal = crate::config::SignalTransport {
                endpoint: &m.endpoint,
                protocol: m.protocol,
                headers: &m.headers,
                timeout_ms: m.timeout_ms,
            };
            let exporter = build_metric_exporter(&signal)?;
            let reader = PeriodicReader::builder(exporter, Tokio)
                .with_interval(Duration::from_millis(m.export_interval_ms))
                .build();
            let provider = SdkMeterProvider::builder()
                .with_reader(reader)
                .with_resource(resource.clone())
                .build();
            global::set_meter_provider(provider.clone());
            Some(provider)
        } else {
            None
        };

        let logger_provider = if let Some(ref l) = config.otlp_logs {
            let signal = crate::config::SignalTransport {
                endpoint: &l.endpoint,
                protocol: l.protocol,
                headers: &l.headers,
                timeout_ms: l.timeout_ms,
            };
            let exporter = build_log_exporter(&signal)?;
            let processor = BatchLogProcessor::builder(exporter).build();
            let provider = SdkLoggerProvider::builder()
                .with_log_processor(processor)
                .with_resource(resource)
                .build();
            Some(provider)
        } else {
            None
        };

        Ok::<_, anyhow::Error>((tracer_provider, meter_provider, logger_provider))
    })?;

    let sls_log_handle =
        install_dispatcher(&config, tracer_provider.as_ref(), logger_provider.as_ref())?;

    global::set_text_map_propagator(TraceContextPropagator::new());

    *guard = Some(RuntimeState {
        tracer_provider,
        meter_provider,
        logger_provider,
        sls_log_handle,
    });
    Ok(())
}

fn install_dispatcher(
    config: &ResolvedConfig,
    tracer_provider: Option<&SdkTracerProvider>,
    logger_provider: Option<&SdkLoggerProvider>,
) -> Result<Option<SlsLogHandle>> {
    use tracing_subscriber::Layer;

    if GLOBAL_DISPATCH_INSTALLED.get().is_some() {
        tracing::warn!(
            "pytracingx: tracing dispatcher already installed; the first init() wins."
        );
        return Ok(None);
    }

    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();

    if config.console_output {
        let filter = build_env_filter(config)?;
        layers.push(build_fmt_layer(&config.console_format).with_filter(filter).boxed());
    }
    if let Some(tp) = tracer_provider {
        let tracer = tp.tracer("pytracingx");
        let layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_tracked_inactivity(false)
            .with_threads(false);
        layers.push(layer.boxed());
    }
    if let Some(lp) = logger_provider {
        layers.push(OpenTelemetryTracingBridge::new(lp).boxed());
    }

    let sls_handle = if let Some(ref sls_cfg) = config.sls_log {
        let (sls_layer, handle) = sls_log::create_sls_log_layer(sls_cfg, &config.service_name)?;
        layers.push(sls_layer.boxed());
        Some(handle)
    } else {
        None
    };

    Registry::default()
        .with(layers)
        .try_init()
        .map_err(|e| anyhow!(format!("set_global_default failed: {e}")))?;

    let _ = GLOBAL_DISPATCH_INSTALLED.set(());
    Ok(sls_handle)
}

fn build_env_filter(config: &ResolvedConfig) -> Result<EnvFilter> {
    if let Some(custom) = &config.log_filter {
        return EnvFilter::try_new(custom)
            .map_err(|e| anyhow!(format!("invalid log_filter '{custom}': {e}")));
    }
    EnvFilter::try_new(&config.console_level)
        .map_err(|e| anyhow!(format!("EnvFilter build failed: {e}")))
}

fn build_fmt_layer(
    format: &str,
) -> Box<dyn tracing_subscriber::Layer<Registry> + Send + Sync> {
    use tracing_subscriber::fmt;
    match format {
        "json" => Box::new(fmt::layer().json().with_target(true).with_writer(std::io::stderr)),
        "pretty" => Box::new(fmt::layer().pretty().with_writer(std::io::stderr)),
        _ => Box::new(fmt::layer().compact().with_writer(std::io::stderr)),
    }
}

pub fn uninstall() -> Result<()> {
    let mut guard = cell().lock();
    let Some(state) = guard.take() else {
        return Ok(());
    };

    if let Some(handle) = &state.sls_log_handle {
        handle.flush_and_shutdown();
    }

    let runtime = pyo3_async_runtimes::tokio::get_runtime();

    // Give in-flight async exports a grace period to complete before
    // tearing down providers. The batch processors export on their own
    // schedule; this sleep lets the last batch finish naturally.
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    // shutdown() internally attempts a final flush. Errors here are
    // expected (backend unreachable at process exit) and not actionable.
    if let Some(tp) = &state.tracer_provider {
        let _ = tp.shutdown();
    }
    if let Some(mp) = &state.meter_provider {
        let _ = mp.shutdown();
    }
    if let Some(lp) = &state.logger_provider {
        let _ = lp.shutdown();
    }
    Ok(())
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

pub fn force_flush() -> Result<()> {
    let guard = cell().lock();
    let Some(state) = guard.as_ref() else {
        return Ok(());
    };
    if let Some(Err(e)) = state.tracer_provider.as_ref().map(|tp| tp.force_flush()) {
        tracing::warn!("trace force_flush error: {e:?}");
    }
    if let Some(Err(e)) = state.meter_provider.as_ref().map(|mp| mp.force_flush()) {
        tracing::warn!("metric force_flush error: {e:?}");
    }
    if let Some(Err(e)) = state.logger_provider.as_ref().map(|lp| lp.force_flush()) {
        tracing::warn!("log force_flush error: {e:?}");
    }
    Ok(())
}
