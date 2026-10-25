use crate::{SpanId, TraceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<i64> for AttributeValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<f64> for AttributeValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<bool> for AttributeValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    Ok,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub service_name: String,
    pub name: String,
    pub start_time_unix_nano: u64,
    pub end_time_unix_nano: u64,
    pub status: SpanStatus,
    pub attributes: BTreeMap<String, AttributeValue>,
}

impl Span {
    pub fn duration_ns(&self) -> u64 {
        self.end_time_unix_nano.saturating_sub(self.start_time_unix_nano)
    }

    pub fn duration_ms(&self) -> f64 {
        self.duration_ns() as f64 / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_duration_and_json_roundtrip() {
        let trace_id: TraceId = "4bf92f3577b34da6a3ce929d0e0e4736".parse().unwrap();
        let span_id: SpanId = "00f067aa0ba902b7".parse().unwrap();

        let mut attributes = BTreeMap::new();
        attributes.insert("http.method".to_string(), AttributeValue::from("POST"));
        attributes.insert("http.status_code".to_string(), AttributeValue::from(200_i64));
        attributes.insert("cache.hit".to_string(), AttributeValue::from(true));

        let span = Span {
            trace_id,
            span_id,
            parent_span_id: None,
            service_name: "checkout-service".to_string(),
            name: "POST /api/checkout".to_string(),
            start_time_unix_nano: 1_700_000_000_000_000_000,
            end_time_unix_nano: 1_700_000_000_045_000_000,
            status: SpanStatus::Ok,
            attributes,
        };

        assert_eq!(span.duration_ns(), 45_000_000);
        assert_eq!(span.duration_ms(), 45.0);

        let json = serde_json::to_string_pretty(&span).expect("should serialize span");
        assert!(json.contains(r#""http.status_code": 200"#));
        assert!(json.contains(r#""http.method": "POST""#));

        let decoded: Span = serde_json::from_str(&json).expect("should deserialize span");
        assert_eq!(decoded, span);
    }
}
