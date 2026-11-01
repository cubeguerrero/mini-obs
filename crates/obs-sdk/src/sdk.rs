use crate::context::{CURRENT_CONTEXT, current_trace_context, new_unix_nano};
use crate::exporter::{BatchConfig, TelemetryExporter, TelemetryItem, run_batch_worker};
use obs_proto::{
    Exemplar, LogRecord, LogSeverity, MetricKind, MetricSample, Span, SpanStatus, TraceContext,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ObsSdk {
    pub service_name: String,
    tx: mpsc::Sender<TelemetryItem>,
    pub dropped_items: Arc<AtomicU64>,
}

impl ObsSdk {
    pub fn new<E: TelemetryExporter>(service_name: &str, exporter: E, config: BatchConfig) -> Self {
        let (tx, rx) = mpsc::channel(config.channel_capacity);

        tokio::spawn(run_batch_worker(rx, exporter, config));

        Self {
            service_name: service_name.to_string(),
            tx,
            dropped_items: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn enqueue(&self, item: TelemetryItem) {
        if self.tx.try_send(item).is_err() {
            self.dropped_items.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub async fn in_span<F, T>(&self, name: &str, fut: F) -> T
    where
        F: Future<Output = T>,
    {
        let (context, parent_span_id) = match current_trace_context() {
            Some(parent) => (parent.new_child(), Some(parent.span_id)),
            None => (TraceContext::new_root(), None),
        };
        let start_time_unix_nano = new_unix_nano();
        let output = CURRENT_CONTEXT.scope(context, fut).await;
        let end_time_unix_nano = new_unix_nano();

        let span = Span {
            trace_id: context.trace_id,
            span_id: context.span_id,
            parent_span_id,
            service_name: self.service_name.clone(),
            name: name.to_string(),
            start_time_unix_nano,
            end_time_unix_nano,
            status: SpanStatus::Ok,
            attributes: BTreeMap::new(),
        };

        self.enqueue(TelemetryItem::Span(span));
        output
    }

    pub fn log(&self, severity: LogSeverity, body: &str) {
        let ctx = current_trace_context();
        let record = LogRecord {
            trace_id: ctx.map(|c| c.trace_id),
            span_id: ctx.map(|c| c.span_id),
            service_name: self.service_name.clone(),
            body: body.to_string(),
            timestamp_unix_nano: new_unix_nano(),
            severity,
            attributes: BTreeMap::new(),
        };
        self.enqueue(TelemetryItem::Log(record));
    }

    pub fn record_metric(
        &self,
        name: &str,
        kind: MetricKind,
        value: f64,
        labels: BTreeMap<String, String>,
    ) {
        let ctx = current_trace_context();
        let timestamp = new_unix_nano();
        let record = MetricSample {
            name: name.to_string(),
            kind,
            labels,
            timestamp_unix_ms: timestamp,
            value,
            exemplar: ctx.map(|c| Exemplar {
                trace_id: c.trace_id,
                span_id: c.span_id,
                value,
                timestamp_unix_ms: timestamp,
            }),
        };
        self.enqueue(TelemetryItem::Metric(record));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exporter::InMemoryExporter;
    use std::time::Duration;

    #[tokio::test]
    async fn test_nested_spans_propagate_context_and_export() {
        let exporter = InMemoryExporter::new();
        let config = BatchConfig {
            channel_capacity: 128,
            max_batch_size: 2,
            flush_interval: Duration::from_millis(50),
        };
        let sdk = ObsSdk::new("checkout", exporter.clone(), config);

        assert_eq!(current_trace_context(), None);

        let result = sdk
            .in_span("POST /pay", async {
                assert!(current_trace_context().is_some());

                sdk.in_span("db.insert_payment", async { 42 }).await
            })
            .await;

        assert_eq!(result, 42);
        assert_eq!(current_trace_context(), None);

        tokio::time::sleep(Duration::from_millis(20)).await;

        let spans = exporter.spans();
        assert_eq!(spans.len(), 2);

        let child_span = &spans[0];
        let root_span = &spans[1];

        assert_eq!(child_span.name, "db.insert_payment");
        assert_eq!(root_span.name, "POST /pay");

        assert_eq!(root_span.parent_span_id, None);
        assert_eq!(child_span.trace_id, root_span.trace_id);
        assert_eq!(child_span.parent_span_id, Some(root_span.span_id));
        assert_ne!(child_span.span_id, root_span.span_id);
    }

    #[tokio::test]
    async fn test_non_blocking_drop_when_channel_full() {
        let exporter = InMemoryExporter::new();
        let config = BatchConfig {
            channel_capacity: 1,
            max_batch_size: 100,
            flush_interval: Duration::from_secs(60),
        };
        let sdk = ObsSdk::new("checkout", exporter, config);

        for _ in 0..10 {
            sdk.in_span("op", async {}).await;
        }

        assert!(sdk.dropped_items.load(Ordering::Relaxed) > 0);
    }

    #[tokio::test]
    async fn test_logging() {
        let exporter = InMemoryExporter::new();
        let config = BatchConfig {
            channel_capacity: 4,
            max_batch_size: 100,
            flush_interval: Duration::from_secs(60),
        };
        let sdk = ObsSdk::new("checkout", exporter.clone(), config);

        sdk.in_span("POST /pay", async {
            sdk.log(LogSeverity::Info, "POST /pay called");
            sdk.in_span("db.insert_payment", async {
                sdk.log(LogSeverity::Info, "Calling database");
            })
            .await;
        })
        .await;

        tokio::time::sleep(Duration::from_millis(20)).await;

        let spans = exporter.spans();
        assert_eq!(spans.len(), 2);

        let child_span = &spans[0];
        let root_span = &spans[1];

        let logs = exporter.logs();
        assert_eq!(logs.len(), 2);

        let root_span_log = &logs[0];
        assert_eq!(root_span_log.body, "POST /pay called");
        assert_eq!(root_span_log.severity, LogSeverity::Info);
        assert_eq!(root_span_log.trace_id.unwrap(), root_span.trace_id);
        assert_eq!(root_span_log.span_id.unwrap(), root_span.span_id);

        let child_span_log = &logs[1];
        assert_eq!(child_span_log.body, "Calling database");
        assert_eq!(child_span_log.severity, LogSeverity::Info);
        assert_eq!(child_span_log.trace_id.unwrap(), child_span.trace_id);
        assert_eq!(child_span_log.span_id.unwrap(), child_span.span_id);
    }

    #[tokio::test]
    async fn test_sending_metric() {
        let exporter = InMemoryExporter::new();
        let config = BatchConfig {
            channel_capacity: 4,
            max_batch_size: 100,
            flush_interval: Duration::from_secs(60),
        };
        let sdk = ObsSdk::new("checkout", exporter.clone(), config);

        sdk.in_span("POST /pay", async {
            let mut labels = BTreeMap::new();
            labels.insert("path".to_string(), "pay".to_string());
            labels.insert("method".to_string(), "post".to_string());

            sdk.record_metric("web.requests.latency", MetricKind::Gauge, 100.0, labels);
            sdk.in_span("db.insert_payment", async {
                let mut labels = BTreeMap::new();
                labels.insert("status".to_string(), "ok".to_string());
                sdk.record_metric("db.insert.latency", MetricKind::Gauge, 100.0, labels);
            })
            .await;
        })
        .await;

        tokio::time::sleep(Duration::from_millis(20)).await;

        let spans = exporter.spans();
        assert_eq!(spans.len(), 2);

        let child_span = &spans[0];
        let root_span = &spans[1];

        let metric_samples = exporter.metrics();
        let root_metric = &metric_samples[0];
        let child_metric = &metric_samples[1];

        assert_eq!(root_metric.name, "web.requests.latency");
        assert_eq!(root_metric.value, 100.0);
        let root_exemplar = root_metric
            .exemplar
            .as_ref()
            .expect("should have root exemplar");
        assert_eq!(root_exemplar.trace_id, root_span.trace_id);
        assert_eq!(root_exemplar.span_id, root_span.span_id);

        assert_eq!(child_metric.name, "db.insert.latency");
        assert_eq!(child_metric.value, 100.0);
        let child_exemplar = child_metric
            .exemplar
            .as_ref()
            .expect("should have child exemplar");
        assert_eq!(child_exemplar.trace_id, child_span.trace_id);
        assert_eq!(child_exemplar.span_id, child_span.span_id);
    }
}
