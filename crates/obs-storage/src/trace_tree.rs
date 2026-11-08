use obs_proto::{Span, SpanId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanNode {
    pub span: Span,
    pub children: Vec<SpanNode>,
    pub self_time_ns: u64,
}

pub fn build_trace_tree(spans: &[Span]) -> Vec<SpanNode> {
    let known_ids: HashSet<SpanId> = spans.iter().map(|s| s.span_id).collect();
    let mut roots: Vec<&Span> = Vec::new();
    let mut children_by_parent: HashMap<SpanId, Vec<&Span>> = HashMap::new();

    for span in spans {
        if let Some(parent_id) = span.parent_span_id
            && known_ids.contains(&parent_id)
        {
            children_by_parent.entry(parent_id).or_default().push(span);
        } else {
            roots.push(span);
        }
    }

    roots.sort_by_key(|s| s.start_time_unix_nano);
    roots
        .into_iter()
        .map(|root| build_node(root, &children_by_parent))
        .collect()
}

fn build_node(span: &Span, children_by_parent: &HashMap<SpanId, Vec<&Span>>) -> SpanNode {
    let children: Vec<SpanNode> = match children_by_parent.get(&span.span_id) {
        Some(child_spans) => {
            let mut sorted = child_spans.clone();
            sorted.sort_by_key(|s| s.start_time_unix_nano);
            sorted
                .into_iter()
                .map(|child| build_node(child, children_by_parent))
                .collect()
        }
        None => Vec::new(),
    };

    let children_duration: u64 = children.iter().map(|c| c.span.duration_ns()).sum();
    let self_time_ns = span.duration_ns().saturating_sub(children_duration);
    SpanNode {
        span: span.clone(),
        children,
        self_time_ns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obs_proto::{SpanId, SpanStatus, TraceId};
    use std::collections::BTreeMap;

    #[test]
    fn test_build_trace_tree() {
        let trace_id = TraceId::generate();
        // Root: 100..500 (duration = 400ns)
        let parent_span = dummy_span(trace_id, None, 100, 500);
        // Child 1 (leaf): 110..160 (duration = 50ns)
        let child_span_without_offspring =
            dummy_span(trace_id, Some(parent_span.span_id), 110, 160);
        // Child 2 (has grandchild): 200..450 (duration = 250ns)
        let child_span_with_offspring = dummy_span(trace_id, Some(parent_span.span_id), 200, 450);
        // Grandchild: 220..320 (duration = 100ns)
        let grandchild_span =
            dummy_span(trace_id, Some(child_span_with_offspring.span_id), 220, 320);

        // Pass spans in out-of-order sequence (grandchild first, root last!)
        let spans = vec![
            grandchild_span.clone(),
            child_span_with_offspring.clone(),
            child_span_without_offspring.clone(),
            parent_span.clone(),
        ];
        let result = build_trace_tree(&spans);

        // 1. Only 1 root node in the returned Vec
        assert_eq!(result.len(), 1);
        let root_node = &result[0];
        assert_eq!(root_node.span.span_id, parent_span.span_id);
        // Root duration is 400ns, minus direct children (50ns + 250ns = 300ns) => 100ns self-time
        assert_eq!(root_node.self_time_ns, 100);

        // 2. Root has 2 children, sorted chronologically by start_time_unix_nano
        assert_eq!(root_node.children.len(), 2);
        let child1 = &root_node.children[0];
        let child2 = &root_node.children[1];

        assert_eq!(child1.span.span_id, child_span_without_offspring.span_id);
        assert_eq!(child1.children.len(), 0);
        assert_eq!(child1.self_time_ns, 50);

        assert_eq!(child2.span.span_id, child_span_with_offspring.span_id);
        // Child 2 duration is 250ns, minus grandchild (100ns) => 150ns self-time
        assert_eq!(child2.self_time_ns, 150);

        // 3. Child 2 has 1 child (the grandchild)
        assert_eq!(child2.children.len(), 1);
        let grandchild = &child2.children[0];
        assert_eq!(grandchild.span.span_id, grandchild_span.span_id);
        assert_eq!(grandchild.children.len(), 0);
        assert_eq!(grandchild.self_time_ns, 100);
    }

    fn dummy_span(
        trace_id: TraceId,
        parent_span_id: Option<SpanId>,
        start_time_unix_nano: u64,
        end_time_unix_nano: u64,
    ) -> Span {
        Span {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id,
            service_name: "test".to_string(),
            name: "op".to_string(),
            start_time_unix_nano,
            end_time_unix_nano,
            status: SpanStatus::Ok,
            attributes: BTreeMap::new(),
        }
    }
}
