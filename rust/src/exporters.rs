use std::time::Duration;

use opentelemetry_otlp::{
    Compression as OtlpCompression, LogExporter, MetricExporter, SpanExporter, WithExportConfig,
    WithHttpConfig, WithTonicConfig,
};
use tonic::transport::ClientTlsConfig;

use crate::config::{Compression, Protocol, SignalTransport, Temporality};
use crate::error::Result;
use crate::sls::{endpoint_host, headers_to_metadata};

fn tls_for(endpoint: &str) -> Result<ClientTlsConfig> {
    let host = endpoint_host(endpoint)?;
    Ok(ClientTlsConfig::new().domain_name(host).with_native_roots())
}

fn map_compression(c: Compression) -> OtlpCompression {
    match c {
        Compression::Gzip => OtlpCompression::Gzip,
        Compression::Zstd => OtlpCompression::Zstd,
    }
}

fn map_temporality(t: Temporality) -> opentelemetry_sdk::metrics::Temporality {
    match t {
        Temporality::Cumulative => opentelemetry_sdk::metrics::Temporality::Cumulative,
        Temporality::Delta => opentelemetry_sdk::metrics::Temporality::Delta,
        Temporality::LowMemory => opentelemetry_sdk::metrics::Temporality::LowMemory,
    }
}

pub fn build_span_exporter(signal: &SignalTransport<'_>) -> Result<SpanExporter> {
    match signal.protocol {
        Protocol::Grpc => {
            let mut b = SpanExporter::builder()
                .with_tonic()
                .with_endpoint(signal.endpoint)
                .with_metadata(headers_to_metadata(signal.headers)?)
                .with_tls_config(tls_for(signal.endpoint)?);
            if let Some(t) = signal.timeout_ms {
                b = b.with_timeout(Duration::from_millis(t));
            }
            if let Some(c) = signal.raw_otlp.compression {
                b = b.with_compression(map_compression(c));
            }
            b.build()
                .map_err(|e| anyhow::anyhow!(format!("span/grpc: {e}")))
        }
        Protocol::HttpProtobuf => {
            let mut b = SpanExporter::builder()
                .with_http()
                .with_endpoint(signal.endpoint)
                .with_headers(signal.headers.clone());
            if let Some(t) = signal.timeout_ms {
                b = b.with_timeout(Duration::from_millis(t));
            }
            if let Some(c) = signal.raw_otlp.compression {
                b = b.with_compression(map_compression(c));
            }
            b.build()
                .map_err(|e| anyhow::anyhow!(format!("span/http: {e}")))
        }
    }
}

pub fn build_metric_exporter(signal: &SignalTransport<'_>) -> Result<MetricExporter> {
    match signal.protocol {
        Protocol::Grpc => {
            let mut b = MetricExporter::builder()
                .with_tonic()
                .with_endpoint(signal.endpoint)
                .with_metadata(headers_to_metadata(signal.headers)?)
                .with_tls_config(tls_for(signal.endpoint)?);
            if let Some(t) = signal.timeout_ms {
                b = b.with_timeout(Duration::from_millis(t));
            }
            if let Some(c) = signal.raw_otlp.compression {
                b = b.with_compression(map_compression(c));
            }
            if let Some(t) = signal.temporality {
                b = b.with_temporality(map_temporality(t));
            }
            b.build()
                .map_err(|e| anyhow::anyhow!(format!("metric/grpc: {e}")))
        }
        Protocol::HttpProtobuf => {
            let mut b = MetricExporter::builder()
                .with_http()
                .with_endpoint(signal.endpoint)
                .with_headers(signal.headers.clone());
            if let Some(t) = signal.timeout_ms {
                b = b.with_timeout(Duration::from_millis(t));
            }
            if let Some(c) = signal.raw_otlp.compression {
                b = b.with_compression(map_compression(c));
            }
            if let Some(t) = signal.temporality {
                b = b.with_temporality(map_temporality(t));
            }
            b.build()
                .map_err(|e| anyhow::anyhow!(format!("metric/http: {e}")))
        }
    }
}

pub fn build_log_exporter(signal: &SignalTransport<'_>) -> Result<LogExporter> {
    match signal.protocol {
        Protocol::Grpc => {
            let mut b = LogExporter::builder()
                .with_tonic()
                .with_endpoint(signal.endpoint)
                .with_metadata(headers_to_metadata(signal.headers)?)
                .with_tls_config(tls_for(signal.endpoint)?);
            if let Some(t) = signal.timeout_ms {
                b = b.with_timeout(Duration::from_millis(t));
            }
            if let Some(c) = signal.raw_otlp.compression {
                b = b.with_compression(map_compression(c));
            }
            b.build()
                .map_err(|e| anyhow::anyhow!(format!("log/grpc: {e}")))
        }
        Protocol::HttpProtobuf => {
            let mut b = LogExporter::builder()
                .with_http()
                .with_endpoint(signal.endpoint)
                .with_headers(signal.headers.clone());
            if let Some(t) = signal.timeout_ms {
                b = b.with_timeout(Duration::from_millis(t));
            }
            if let Some(c) = signal.raw_otlp.compression {
                b = b.with_compression(map_compression(c));
            }
            b.build()
                .map_err(|e| anyhow::anyhow!(format!("log/http: {e}")))
        }
    }
}
