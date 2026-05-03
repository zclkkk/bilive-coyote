use crate::coyote::manager::CoyoteSharedHandle;
use crate::coyote::protocol::*;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

pub async fn coyote_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<CoyoteServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_coyote_socket(socket, state))
}

pub struct CoyoteServerState {
    pub bridge_id: String,
    pub shared: Arc<CoyoteSharedHandle>,
}

async fn handle_coyote_socket(socket: WebSocket, state: Arc<CoyoteServerState>) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<String>(32);
    let client_id = uuid::Uuid::new_v4().to_string();

    let bind_msg = build_message("bind", &client_id, "", "targetId");
    if out_tx.try_send(bind_msg).is_err() {
        return;
    }

    info!("[Coyote] App connected: {client_id}");

    let sink_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_stream.next().await {
        match msg {
            Message::Text(text) => {
                let parsed = parse_message(&text);
                match parsed {
                    Ok(coyote_msg) => {
                        if coyote_msg.msg_type.as_str() == Some("bind") {
                            if !state.shared.bridge_id_matches(&coyote_msg.client_id) {
                                let resp = build_message(
                                    "bind",
                                    &coyote_msg.client_id,
                                    &coyote_msg.target_id,
                                    ERR_INVALID_QR_CLIENT_ID,
                                );
                                let _ = out_tx.try_send(resp);
                                continue;
                            }
                            if coyote_msg.target_id != client_id {
                                let resp = build_message(
                                    "bind",
                                    &coyote_msg.client_id,
                                    &coyote_msg.target_id,
                                    ERR_NO_TARGET_ID,
                                );
                                let _ = out_tx.try_send(resp);
                                continue;
                            }

                            let old_tx =
                                state.shared.register_app(client_id.clone(), out_tx.clone());
                            if let Some(old) = old_tx {
                                let _ = old
                                    .send(build_message(
                                        "error",
                                        "",
                                        "",
                                        ERR_PEER_DISCONNECTED,
                                    ))
                                    .await;
                            }

                            let resp =
                                build_message("bind", &state.bridge_id, &client_id, ERR_SUCCESS);
                            let _ = out_tx.try_send(resp);
                            info!("[Coyote] Paired with app: {client_id}");
                        } else {
                            let is_paired = state.shared.is_paired_app(&client_id)
                                && state.shared.bridge_id_matches(&coyote_msg.client_id);

                            if !is_paired {
                                let resp = build_message(
                                    "error",
                                    &coyote_msg.client_id,
                                    &coyote_msg.target_id,
                                    ERR_NOT_PAIRED,
                                );
                                let _ = out_tx.try_send(resp);
                                continue;
                            }

                            state.shared.handle_app_message(&coyote_msg.message);
                        }
                    }
                    Err(err) => {
                        let resp = build_message("msg", "", "", err.code);
                        let _ = out_tx.try_send(resp);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    state.shared.disconnect_app();
    sink_task.abort();
    info!("[Coyote] App disconnected: {client_id}");
}
