use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aliyun_log_rust_sdk::{Client, Config as SlsConfig, FromConfig};
use aliyun_log_sdk_protobuf::{Log, LogGroup};
use tokio::sync::mpsc;
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use crate::config::ResolvedSlsLogSink;
use crate::error::Result;

const BATCH_SIZE: usize = 4096;
const FLUSH_INTERVAL: Duration = Duration::from_secs(3);

struct LogEntry {
    time: u32,
    level: &'static str,
    message: String,
    logger_name: String,
    attributes: String,
    service_name: String,
    trace_id: String,
    span_id: String,
}

pub struct SlsLogLayer {
    tx: mpsc::UnboundedSender<LogEntry>,
    service_name: String,
}

pub struct SlsLogHandle {
    shutdown_tx: mpsc::Sender<()>,
}

impl SlsLogHandle {
    pub fn flush_and_shutdown(&self) {
        let _ = self.shutdown_tx.try_send(());
    }
}

pub fn create_sls_log_layer(
    cfg: &ResolvedSlsLogSink,
    service_name: &str,
) -> Result<(SlsLogLayer, SlsLogHandle)> {
    let sls_config = SlsConfig::builder()
        .endpoint(&cfg.endpoint)
        .access_key(&cfg.ak_id, &cfg.ak_secret)
        .build()
        .map_err(|e| anyhow::anyhow!(format!("SLS client config error: {e}")))?;
    let client = Client::from_config(sls_config)
        .map_err(|e| anyhow::anyhow!(format!("SLS client build error: {e}")))?;

    let (tx, rx) = mpsc::unbounded_channel::<LogEntry>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

    let project = cfg.project.clone();
    let logstore = cfg.logstore.clone();
    let topic = cfg.topic.clone();
    let source = cfg.source.clone();

    tokio::spawn(background_writer(
        client, rx, shutdown_rx, project, logstore, topic, source,
    ));

    let layer = SlsLogLayer {
        tx,
        service_name: service_name.to_string(),
    };
    let handle = SlsLogHandle { shutdown_tx };
    Ok((layer, handle))
}

async fn background_writer(
    client: Client,
    mut rx: mpsc::UnboundedReceiver<LogEntry>,
    mut shutdown_rx: mpsc::Receiver<()>,
    project: String,
    logstore: String,
    topic: String,
    source: String,
) {
    let mut batch: Vec<LogEntry> = Vec::with_capacity(BATCH_SIZE);
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            entry = rx.recv() => {
                match entry {
                    Some(e) => {
                        batch.push(e);
                        if batch.len() >= BATCH_SIZE {
                            flush_batch(&client, &project, &logstore, &topic, &source, &mut batch).await;
                        }
                    }
                    None => {
                        // Channel closed (runtime dropped) → final flush
                        flush_batch(&client, &project, &logstore, &topic, &source, &mut batch).await;
                        return;
                    }
                }
            }
            _ = interval.tick() => {
                if !batch.is_empty() {
                    flush_batch(&client, &project, &logstore, &topic, &source, &mut batch).await;
                }
            }
            _ = shutdown_rx.recv() => {
                // Drain remaining items from the channel
                while let Ok(e) = rx.try_recv() {
                    batch.push(e);
                }
                flush_batch(&client, &project, &logstore, &topic, &source, &mut batch).await;
                return;
            }
        }
    }
}

async fn flush_batch(
    client: &Client,
    project: &str,
    logstore: &str,
    topic: &str,
    source: &str,
    batch: &mut Vec<LogEntry>,
) {
    if batch.is_empty() {
        return;
    }

    let mut log_group = LogGroup::new();
    if !topic.is_empty() {
        log_group.set_topic(topic.to_string());
    }
    if !source.is_empty() {
        log_group.set_source(source.to_string());
    }

    for entry in batch.drain(..) {
        let mut log = Log::from_unixtime(entry.time);
        log.add_content_kv("level", entry.level);
        log.add_content_kv("message", &entry.message);
        if !entry.logger_name.is_empty() {
            log.add_content_kv("logger_name", &entry.logger_name);
        }
        if !entry.service_name.is_empty() {
            log.add_content_kv("service.name", &entry.service_name);
        }
        if !entry.trace_id.is_empty() {
            log.add_content_kv("trace_id", &entry.trace_id);
        }
        if !entry.span_id.is_empty() {
            log.add_content_kv("span_id", &entry.span_id);
        }
        if !entry.attributes.is_empty() {
            log.add_content_kv("attributes", &entry.attributes);
        }
        log_group.add_log(log);
    }

    if let Err(e) = client
        .put_logs(project, logstore)
        .log_group(log_group)
        .send()
        .await
    {
        tracing::warn!(target: "pytracingx::sls_log", "SLS put_logs failed: {e}");
    }
}

impl<S> Layer<S> for SlsLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = match *meta.level() {
            tracing::Level::TRACE => "TRACE",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::INFO => "INFO",
            tracing::Level::WARN => "WARN",
            tracing::Level::ERROR => "ERROR",
        };

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        // Get trace/span ids from the current tracing span via OpenTelemetrySpanExt
        use opentelemetry::trace::TraceContextExt;
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        let current_span = tracing::Span::current();
        let (trace_id, span_id) = {
            let otel_ctx = current_span.context();
            let sc = otel_ctx.span().span_context().clone();
            if sc.is_valid() {
                (
                    format!("{:032x}", u128::from_be_bytes(sc.trace_id().to_bytes())),
                    format!("{:016x}", u64::from_be_bytes(sc.span_id().to_bytes())),
                )
            } else {
                (String::new(), String::new())
            }
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        let _ = self.tx.send(LogEntry {
            time: now,
            level,
            message: visitor.message,
            logger_name: visitor.logger_name,
            attributes: visitor.attributes,
            service_name: self.service_name.clone(),
            trace_id,
            span_id,
        });
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    logger_name: String,
    attributes: String,
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "logger_name" => self.logger_name = value.to_string(),
            "attributes" => self.attributes = value.to_string(),
            "severity_text" => {}
            _ => {
                if self.message.is_empty() {
                    self.message = value.to_string();
                }
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }
}
