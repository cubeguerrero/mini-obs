use obs_proto::TraceContext;
use std::time::{SystemTime, UNIX_EPOCH};

tokio::task_local! {
    pub(crate) static CURRENT_CONTEXT: TraceContext;
}

pub fn current_trace_context() -> Option<TraceContext> {
    CURRENT_CONTEXT.try_with(|ctx| *ctx).ok()
}

pub fn new_unix_nano() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub fn new_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
