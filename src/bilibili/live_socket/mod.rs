pub mod json_extract;
pub mod packet;

use crate::bilibili::live_socket::json_extract::extract_json_messages;
use crate::bilibili::live_socket::packet::*;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async_tls_with_config, MaybeTlsStream, WebSocketStream};
use tracing::{error, info, warn};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const RECONNECT_BASE: Duration = Duration::from_secs(3);
const RECONNECT_MAX: Duration = Duration::from_secs(60);
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSocketStatus {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct LiveSocketOptions {
    pub label: String,
    pub urls: Vec<String>,
    pub auth: serde_json::Value,
    pub room_id: Option<u64>,
    pub on_message: tokio::sync::mpsc::Sender<serde_json::Value>,
    pub on_status: tokio::sync::mpsc::Sender<LiveSocketStatus>,
}

pub async fn run_live_socket(opts: LiveSocketOptions, cancel: tokio_util::sync::CancellationToken) {
    if opts.urls.is_empty() {
        let _ = opts
            .on_status
            .send(LiveSocketStatus {
                connected: false,
                room_id: opts.room_id,
                error: Some("wss_link 为空".into()),
            })
            .await;
        return;
    }

    let mut reconnect_attempts = 0;
    let mut url_index = 0usize;

    loop {
        let url = &opts.urls[url_index % opts.urls.len()];
        info!(
            "[{}] Connecting to {url}, room: {:?}",
            opts.label, opts.room_id
        );

        let ws_result = connect_async_tls_with_config(url, None, false, None).await;

        match ws_result {
            Ok((ws_stream, _)) => {
                reconnect_attempts = 0;
                if let Err(e) = handle_connection(ws_stream, &opts, &cancel).await {
                    warn!("[{}] Connection handler error: {e}", opts.label);
                }
            }
            Err(e) => {
                error!("[{}] Connect error: {e}", opts.label);
            }
        }

        if cancel.is_cancelled() {
            let _ = opts
                .on_status
                .send(LiveSocketStatus {
                    connected: false,
                    room_id: opts.room_id,
                    error: None,
                })
                .await;
            return;
        }

        reconnect_attempts += 1;
        if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
            let _ = opts
                .on_status
                .send(LiveSocketStatus {
                    connected: false,
                    room_id: opts.room_id,
                    error: Some(format!(
                        "弹幕连接断开，已重试 {MAX_RECONNECT_ATTEMPTS} 次仍失败，请手动重新连接"
                    )),
                })
                .await;
            return;
        }

        url_index = (url_index + 1) % opts.urls.len();
        let exp = (reconnect_attempts - 1).min(5);
        let delay_ms = (RECONNECT_BASE.as_millis() as u64)
            .saturating_mul(2u64.saturating_pow(exp))
            .min(RECONNECT_MAX.as_millis() as u64);
        let delay = Duration::from_millis(delay_ms);

        info!(
            "[{}] Reconnecting in {:?} (attempt {reconnect_attempts}/{MAX_RECONNECT_ATTEMPTS})",
            opts.label, delay
        );

        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = cancel.cancelled() => {
                let _ = opts.on_status.send(LiveSocketStatus {
                    connected: false,
                    room_id: opts.room_id,
                    error: None,
                }).await;
                return;
            }
        }
    }
}

async fn handle_connection(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    opts: &LiveSocketOptions,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    let auth_body = serde_json::to_string(&opts.auth)?;
    let auth_packet = build_packet(OP_AUTH, &auth_body);
    ws_sink
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            auth_packet.into(),
        ))
        .await?;

    let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat_interval.tick().await;

    loop {
        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(data))) => {
                        handle_data(&data, opts).await;
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        if let Ok(data) = hex::decode(&text) {
                            handle_data(&data, opts).await;
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("[{}] WS error: {e}", opts.label);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            _ = heartbeat_interval.tick() => {
                let hb = build_packet(OP_HEARTBEAT, "");
                if ws_sink.send(tokio_tungstenite::tungstenite::Message::Binary(hb.into())).await.is_err() {
                    break;
                }
            }
            _ = cancel.cancelled() => {
                let _ = ws_sink.close().await;
                return Ok(());
            }
        }
    }

    let _ = opts
        .on_status
        .send(LiveSocketStatus {
            connected: false,
            room_id: opts.room_id,
            error: Some("弹幕连接断开，正在重连".into()),
        })
        .await;

    Ok(())
}

fn collect_messages(protover: u16, body: &[u8], out: &mut Vec<serde_json::Value>) {
    match protover {
        PROTOVER_PLAIN | 0 => {
            out.extend(extract_json_messages(body));
        }
        PROTOVER_DEFLATE | PROTOVER_BROTLI => match decompress_body(protover, body) {
            Ok(decompressed) => {
                let packets = parse_packets(&decompressed);
                for packet in packets {
                    if packet.op == OP_MESSAGE {
                        collect_messages(packet.protover, &packet.body, out);
                    }
                }
            }
            Err(e) => {
                error!("Decompression error: {e}");
            }
        },
        _ => {
            warn!("Unknown protover: {protover}");
        }
    }
}

async fn handle_data(data: &[u8], opts: &LiveSocketOptions) {
    let packets = parse_packets(data);

    for packet in packets {
        match packet.op {
            OP_CONNECT_SUCCESS => {
                info!("[{}] Auth success", opts.label);
                let _ = opts
                    .on_status
                    .send(LiveSocketStatus {
                        connected: true,
                        room_id: opts.room_id,
                        error: None,
                    })
                    .await;
            }
            OP_HEARTBEAT_REPLY => {}
            OP_MESSAGE => {
                let mut msgs = Vec::new();
                collect_messages(packet.protover, &packet.body, &mut msgs);
                for msg in msgs {
                    let _ = opts.on_message.send(msg).await;
                }
            }
            _ => {
                tracing::debug!("[{}] Unknown op: {}", opts.label, packet.op);
            }
        }
    }
}
