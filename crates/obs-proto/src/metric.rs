use crate::{SpanId, TraceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Counter,         // Monotonically increasing (e.g., total requests)
    Gauge,           // Goes up or down (e.g., active connections, memory bytes)
    HistogramSample, // Individual latency/size observation
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exemplar {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub value: f64,
    pub timestamp_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub kind: MetricKind,
    pub timestamp_unix_ms: u64,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exemplar: Option<Exemplar>,
}
