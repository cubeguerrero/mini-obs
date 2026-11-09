use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use obs_proto::{IngestTracesRequest, TraceId};
use obs_storage::{SpanNode, TraceStore};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub trace_store: Arc<RwLock<TraceStore>>,
}

async fn index() -> Html<&'static str> {
    Html(include_str!("ui.html"))
}

async fn ingest_traces(
    State(state): State<AppState>,
    Json(payload): Json<IngestTracesRequest>,
) -> StatusCode {
    let mut store = state.trace_store.write().unwrap();
    for span in payload.spans {
        store.ingest_span(span);
    }
    StatusCode::ACCEPTED
}

async fn list_traces(State(state): State<AppState>) -> Json<Vec<TraceId>> {
    let store = state.trace_store.read().unwrap();
    Json(store.recent_trace_ids())
}

async fn get_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<TraceId>,
) -> Result<Json<Vec<SpanNode>>, StatusCode> {
    let store = state.trace_store.read().unwrap();
    match store.get_trace_tree(&trace_id) {
        Some(tree) => Ok(Json(tree)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/v1/traces", post(ingest_traces))
        .route("/api/traces", get(list_traces))
        .route("/api/traces/{trace_id}", get(get_trace))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let state = AppState {
        trace_store: Arc::new(RwLock::new(TraceStore::new(1000))),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4318")
        .await
        .unwrap();
    println!("obs-server listening on http://127.0.0.1:4318");
    axum::serve(listener, app(state)).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use obs_sdk::{BatchConfig, HttpExporter, ObsSdk};
    use std::time::Duration;

    #[tokio::test]
    async fn integration_test() {
        let state = AppState {
            trace_store: Arc::new(RwLock::new(TraceStore::new(1000))),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());

        tokio::spawn(async move { axum::serve(listener, app(state)).await.unwrap() });

        let exporter = HttpExporter::new(&base_url);
        let config = BatchConfig {
            channel_capacity: 128,
            max_batch_size: 2,
            flush_interval: Duration::from_millis(20),
        };
        let sdk = ObsSdk::new("checkout", exporter.clone(), config);
        sdk.in_span("POST /pay", async {
            sdk.in_span("db.insert_payment", async {}).await;
        })
        .await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let traces: Vec<TraceId> = reqwest::get(format!("{base_url}/api/traces"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        assert_eq!(traces.len(), 1);

        let tree: Vec<SpanNode> = reqwest::get(format!("{base_url}/api/traces/{}", traces[0]))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(tree[0].span.name, "POST /pay");
        assert_eq!(tree[0].children[0].span.name, "db.insert_payment");
    }
}
