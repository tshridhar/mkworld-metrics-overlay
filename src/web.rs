use crate::state::SharedState;
use axum::http::Method;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::{Stream, StreamExt};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

pub fn create_router(state: SharedState) -> Router {
    let static_files = ServeDir::new("public");

    let cors = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_origin(Any);

    Router::new()
        .route("/events", get(sse_handler))
        .fallback_service(static_files)
        .layer(cors)
        .with_state(state)
}

async fn sse_handler(
    State(state): State<SharedState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Send state every 2 seconds via SSE
    let stream =
        IntervalStream::new(tokio::time::interval(Duration::from_secs(2))).then(move |_| {
            let state = state.clone();
            async move {
                let current_state = {
                    let st = state.lock().await;
                    st.state.clone()
                };
                let data = serde_json::json!({ 
                    "race": current_state.current_race,
                    "points": current_state.current_points
                }).to_string();
                Ok(Event::default().data(data))
            }
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive-text"),
    )
}
