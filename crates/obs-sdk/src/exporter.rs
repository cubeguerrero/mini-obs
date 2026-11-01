use obs_proto::{LogRecord, MetricSample, Span};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryItem {
    Span(Span),
    Metric(MetricSample),
    Log(LogRecord),
}

pub trait TelemetryExporter: Send + Sync + 'static {
    fn export(&self, batch: Vec<TelemetryItem>) -> impl Future<Output = ()> + Send;
}

#[derive(Clone, Default)]
pub struct InMemoryExporter {
    pub items: Arc<Mutex<Vec<TelemetryItem>>>,
}

impl InMemoryExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spans(&self) -> Vec<Span> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .filter_map(|item| match item {
                TelemetryItem::Span(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn metrics(&self) -> Vec<MetricSample> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .filter_map(|item| match item {
                TelemetryItem::Metric(m) => Some(m.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn logs(&self) -> Vec<LogRecord> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .filter_map(|item| match item {
                TelemetryItem::Log(l) => Some(l.clone()),
                _ => None,
            })
            .collect()
    }
}

impl TelemetryExporter for InMemoryExporter {
    async fn export(&self, mut batch: Vec<TelemetryItem>) {
        self.items.lock().unwrap().append(&mut batch);
    }
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub channel_capacity: usize,
    pub max_batch_size: usize,
    pub flush_interval: Duration,
}

pub(crate) async fn run_batch_worker<E: TelemetryExporter>(
    mut rx: mpsc::Receiver<TelemetryItem>,
    exporter: E,
    config: BatchConfig,
) {
    let mut batch = Vec::new();
    let mut ticker = tokio::time::interval(config.flush_interval);

    loop {
        tokio::select! {
            maybe_item = rx.recv() => {
                match maybe_item {
                    Some(item) => {
                        batch.push(item);
                        if batch.len() >= config.max_batch_size {
                            exporter.export(std::mem::take(&mut batch)).await;
                        }
                    }
                    None => {
                        if !batch.is_empty() {
                            exporter.export(std::mem::take(&mut batch)).await;
                        }
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                if !batch.is_empty() {
                    exporter.export(std::mem::take(&mut batch)).await;
                }
            }
        }
    }
}
