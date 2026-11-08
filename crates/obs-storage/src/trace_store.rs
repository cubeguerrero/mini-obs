use obs_proto::{Span, TraceId};
use std::collections::{HashMap, VecDeque};

use crate::{SpanNode, build_trace_tree};

pub struct TraceStore {
    max_traces: usize,
    traces: HashMap<TraceId, Vec<Span>>,
    order: VecDeque<TraceId>,
}

impl TraceStore {
    pub fn new(max_traces: usize) -> Self {
        Self {
            max_traces,
            traces: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn ingest_span(&mut self, span: Span) {
        if !self.traces.contains_key(&span.trace_id) {
            if self.traces.len() >= self.max_traces
                && let Some(trace_id) = self.order.pop_front()
            {
                self.traces.remove(&trace_id);
            }
            self.order.push_back(span.trace_id);
        }

        self.traces.entry(span.trace_id).or_default().push(span);
    }

    pub fn get_spans(&self, trace_id: &TraceId) -> Option<&[Span]> {
        self.traces.get(trace_id).map(|v| v.as_slice())
    }

    pub fn get_trace_tree(&self, trace_id: &TraceId) -> Option<Vec<SpanNode>> {
        self.get_spans(trace_id).map(build_trace_tree)
    }

    pub fn recent_trace_ids(&self) -> Vec<TraceId> {
        self.order.iter().rev().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obs_proto::{SpanId, SpanStatus};
    use std::collections::BTreeMap;

    #[test]
    fn test_ingest_span_new() {
        let trace_id: TraceId = "4bf92f3577b34da6a3ce929d0e0e4736".parse().unwrap();
        let span = dummy_span(trace_id);
        let trace_id_2: TraceId = "4bf92f3577b34da6a3ce929d0e0e4737".parse().unwrap();
        let span_2 = dummy_span(trace_id_2);
        let span_3 = dummy_span(trace_id_2);

        let mut trace_store = TraceStore::new(2);
        trace_store.ingest_span(span.clone());
        trace_store.ingest_span(span_2.clone());
        trace_store.ingest_span(span_3.clone());

        let spans_1 = trace_store
            .get_spans(&trace_id)
            .expect("There should be spans");
        assert_eq!(spans_1.len(), 1);

        let spans_2 = trace_store
            .get_spans(&trace_id_2)
            .expect("There should be spans");
        assert_eq!(spans_2.len(), 2);
        assert_eq!(spans_2[0].name, span_2.name);
        assert_eq!(spans_2[1].name, span_3.name);
    }

    #[test]
    fn test_eviction_span() {
        let trace_id: TraceId = "4bf92f3577b34da6a3ce929d0e0e4736".parse().unwrap();
        let span = dummy_span(trace_id);
        let trace_id_2: TraceId = "4bf92f3577b34da6a3ce929d0e0e4737".parse().unwrap();
        let span_2 = dummy_span(trace_id_2);
        let trace_id_3: TraceId = "4bf92f3577b34da6a3ce929d0e0e4738".parse().unwrap();
        let span_3 = dummy_span(trace_id_3);

        let mut trace_store = TraceStore::new(2);
        trace_store.ingest_span(span.clone());
        trace_store.ingest_span(span_2.clone());
        trace_store.ingest_span(span_3.clone());

        assert!(trace_store.get_spans(&trace_id).is_none());
        assert!(trace_store.get_spans(&trace_id_2).is_some());
        assert!(trace_store.get_spans(&trace_id_3).is_some());
    }

    fn dummy_span(trace_id: TraceId) -> Span {
        Span {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id: None,
            service_name: "test".to_string(),
            name: "op".to_string(),
            start_time_unix_nano: 100,
            end_time_unix_nano: 200,
            status: SpanStatus::Ok,
            attributes: BTreeMap::new(),
        }
    }
}
