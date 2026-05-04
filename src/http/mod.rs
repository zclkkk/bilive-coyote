pub mod error;
pub mod routes;
pub mod static_files;

use crate::http::routes::AppState;
use crate::http::static_files::static_handler;
use axum::routing::{get, post, put};
use axum::Router;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/status", get(routes::get_status))
        .route("/api/bilibili/start", post(routes::bilibili_start))
        .route("/api/bilibili/stop", post(routes::bilibili_stop))
        .route("/api/bilibili/status", get(routes::bilibili_status))
        .route("/api/coyote/status", get(routes::coyote_status))
        .route("/api/coyote/qrcode", get(routes::coyote_qrcode))
        .route("/api/coyote/strength", post(routes::coyote_strength))
        .route("/api/coyote/waveforms", get(routes::coyote_waveforms))
        .route("/api/coyote/waveform", post(routes::coyote_waveform))
        .route("/api/coyote/emergency", post(routes::coyote_emergency))
        .route("/api/config", get(routes::get_config))
        .route("/api/config", put(routes::put_config))
        .route("/api/config/rules", get(routes::get_rules))
        .route("/api/config/rules", put(routes::put_rules))
        .route("/ws/panel", get(panel_ws_handler))
        .fallback(static_handler)
        .with_state(state)
}

async fn panel_ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_panel_socket(socket, state.panel_tx.subscribe()))
}

async fn handle_panel_socket(
    socket: axum::extract::ws::WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<crate::engine::types::PanelEvent>,
) {
    use futures_util::{SinkExt, StreamExt};
    let (mut ws_sink, mut ws_stream) = socket.split();

    let write_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let msg = match serde_json::to_string(&event) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if ws_sink
                .send(axum::extract::ws::Message::Text(msg.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_stream.next().await {
        if let axum::extract::ws::Message::Close(_) = msg {
            break;
        }
    }

    write_task.abort();
}
