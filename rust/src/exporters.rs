use std::time::Duration;

use opentelemetry_otlp::{
    LogExporter, MetricExporter, SpanExporter, WithExportConfig, WithHttpConfig, WithTonicConfig,
};
use tonic::transport::ClientTlsConfig;

use crate::config::{Protocol, SignalTransport};
use crate::error::Result;
use crate::sls::{endpoint_host, headers_to_metadata};

fn tls_for(endpoint: &str) -> Result<ClientTlsConfig> {
    let host = endpoint_host(endpoint)?;
    Ok(ClientTlsConfig::new().domain_name(host).with_native_roots())
}

pub fn build_span_exporter(signal: &SignalTransport<'_>) -> Result<SpanExporter> {
    match signal.protocol {
        Protocol::Grpc => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(signal.endpoint)
            .with_timeout(Duration::from_millis(signal.timeout_ms))
            .with_metadata(headers_to_metadata(signal.headers)?)
            .with_tls_config(tls_for(signal.endpoint)?)
            .build()
            .map_err(|e| anyhow::anyhow!(format!("span/grpc: {e}"))),
        Protocol::HttpProtobuf => SpanExporter::builder()
            .with_http()
            .with_endpoint(signal.endpoint)
            .with_timeout(Duration::from_millis(signal.timeout_ms))
            .with_headers(signal.headers.clone())
            .build()
            .map_err(|e| anyhow::anyhow!(format!("span/http: {e}"))),
    }
}

pub fn build_metric_exporter(signal: &SignalTransport<'_>) -> Result<MetricExporter> {
    match signal.protocol {
        Protocol::Grpc => MetricExporter::builder()
            .with_tonic()
            .with_endpoint(signal.endpoint)
            .with_timeout(Duration::from_millis(signal.timeout_ms))
            .with_metadata(headers_to_metadata(signal.headers)?)
            .with_tls_config(tls_for(signal.endpoint)?)
            .build()
            .map_err(|e| anyhow::anyhow!(format!("metric/grpc: {e}"))),
        Protocol::HttpProtobuf => MetricExporter::builder()
            .with_http()
            .with_endpoint(signal.endpoint)
            .with_timeout(Duration::from_millis(signal.timeout_ms))
            .with_headers(signal.headers.clone())
            .build()
            .map_err(|e| anyhow::anyhow!(format!("metric/http: {e}"))),
    }
}

pub fn build_log_exporter(signal: &SignalTransport<'_>) -> Result<LogExporter> {
    match signal.protocol {
        Protocol::Grpc => LogExporter::builder()
            .with_tonic()
            .with_endpoint(signal.endpoint)
            .with_timeout(Duration::from_millis(signal.timeout_ms))
            .with_metadata(headers_to_metadata(signal.headers)?)
            .with_tls_config(tls_for(signal.endpoint)?)
            .build()
            .map_err(|e| anyhow::anyhow!(format!("log/grpc: {e}"))),
        Protocol::HttpProtobuf => LogExporter::builder()
            .with_http()
            .with_endpoint(signal.endpoint)
            .with_timeout(Duration::from_millis(signal.timeout_ms))
            .with_headers(signal.headers.clone())
            .build()
            .map_err(|e| anyhow::anyhow!(format!("log/http: {e}"))),
    }
}
