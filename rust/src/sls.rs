use std::collections::HashMap;

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use tonic::metadata::{MetadataKey, MetadataMap, MetadataValue};

use crate::config::ResolvedConfig;
use crate::error::{PtxError, PtxResult};

pub fn headers_to_metadata(headers: &HashMap<String, String>) -> PtxResult<MetadataMap> {
    let mut map = MetadataMap::with_capacity(headers.len());
    for (k, v) in headers {
        let key = MetadataKey::from_bytes(k.to_ascii_lowercase().as_bytes())
            .map_err(|e| PtxError::Config(format!("invalid header name '{k}': {e}")))?;
        let value = MetadataValue::try_from(v.as_str())
            .map_err(|e| PtxError::Config(format!("invalid header value for '{k}': {e}")))?;
        map.insert(key, value);
    }
    Ok(map)
}

pub fn build_resource(cfg: &ResolvedConfig) -> Resource {
    let mut kvs = Vec::with_capacity(cfg.resource_attributes.len() + 5);
    kvs.push(KeyValue::new("telemetry.sdk.name", "pytracingx"));
    kvs.push(KeyValue::new(
        "telemetry.sdk.version",
        env!("CARGO_PKG_VERSION"),
    ));
    kvs.push(KeyValue::new("telemetry.sdk.language", "rust"));
    kvs.push(KeyValue::new(
        "host.name",
        hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".into()),
    ));
    kvs.push(KeyValue::new(
        "process.pid",
        i64::from(std::process::id() as i32),
    ));
    for (k, v) in &cfg.resource_attributes {
        kvs.push(KeyValue::new(k.clone(), v.clone()));
    }
    Resource::builder()
        .with_service_name(cfg.service_name.clone())
        .with_attributes(kvs)
        .build()
}

pub fn endpoint_host(endpoint: &str) -> PtxResult<String> {
    let url = url::Url::parse(endpoint)
        .map_err(|e| PtxError::Endpoint(format!("could not parse '{endpoint}': {e}")))?;
    url.host_str()
        .map(|s| s.to_string())
        .ok_or_else(|| PtxError::Endpoint(format!("endpoint '{endpoint}' has no host")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_lower_cases_keys() {
        let mut h = HashMap::new();
        h.insert("X-SLS-Otel-Project".to_string(), "p".to_string());
        let mm = headers_to_metadata(&h).unwrap();
        assert!(mm.get("x-sls-otel-project").is_some());
    }

    #[test]
    fn host_extraction() {
        let host = endpoint_host("https://x.cn-hangzhou.log.aliyuncs.com:10010").unwrap();
        assert_eq!(host, "x.cn-hangzhou.log.aliyuncs.com");
    }
}
