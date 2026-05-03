use crate::bilibili::broadcast::parser::parse_broadcast_gift;
use crate::bilibili::broadcast::wbi::fetch_danmu_info;
use crate::bilibili::live_socket::{run_live_socket, LiveSocketOptions, LiveSocketStatus};
use crate::config::types::GiftEvent;
use crate::config::ConfigHandle;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

pub struct BroadcastSource {
    config: ConfigHandle,
    gift_tx: mpsc::Sender<GiftEvent>,
    status_tx: mpsc::Sender<LiveSocketStatus>,
    cancel: Arc<std::sync::Mutex<tokio_util::sync::CancellationToken>>,
}

impl BroadcastSource {
    pub fn new(
        config: ConfigHandle,
        gift_tx: mpsc::Sender<GiftEvent>,
        status_tx: mpsc::Sender<LiveSocketStatus>,
    ) -> Self {
        Self {
            config,
            gift_tx,
            status_tx,
            cancel: Arc::new(std::sync::Mutex::new(
                tokio_util::sync::CancellationToken::new(),
            )),
        }
    }

    fn reset_cancel(&self) -> tokio_util::sync::CancellationToken {
        let mut guard = self.cancel.lock().unwrap();
        guard.cancel();
        let new = tokio_util::sync::CancellationToken::new();
        *guard = new.clone();
        new
    }

    pub async fn start(&self, room_id: Option<u64>) -> Result<(), String> {
        let cancel = self.reset_cancel();

        let cfg = self.config.lock().await;
        let requested = room_id.unwrap_or(cfg.get().bilibili.broadcast.room_id);
        drop(cfg);

        if requested == 0 {
            return Err("roomId required".into());
        }

        let (key, urls, long_room_id) = fetch_danmu_info(requested).await?;

        let gift_tx = self.gift_tx.clone();
        let status_tx = self.status_tx.clone();

        let auth = serde_json::json!({
            "uid": 0,
            "roomid": long_room_id,
            "protover": 3,
            "platform": "web",
            "type": 2,
            "key": key,
        });

        let (msg_tx, mut msg_rx) = mpsc::channel::<serde_json::Value>(256);
        let (inner_status_tx, mut inner_status_rx) = mpsc::channel::<LiveSocketStatus>(16);

        let ls_cancel = cancel.clone();
        tokio::spawn(async move {
            run_live_socket(
                LiveSocketOptions {
                    label: "Bilibili/Broadcast".into(),
                    urls,
                    auth,
                    room_id: Some(long_room_id),
                    on_message: msg_tx,
                    on_status: inner_status_tx,
                },
                ls_cancel,
            )
            .await;
        });

        let gift_tx_h = gift_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                if let Some(gift) = parse_broadcast_gift(&msg) {
                    let _ = gift_tx_h.send(gift).await;
                }
            }
        });

        let status_tx_h = status_tx.clone();
        tokio::spawn(async move {
            while let Some(status) = inner_status_rx.recv().await {
                let _ = status_tx_h.send(status).await;
            }
        });

        self.config
            .lock()
            .await
            .update(serde_json::json!({
                "bilibili": {
                    "source": "broadcast",
                    "broadcast": { "roomId": long_room_id }
                }
            }))
            .await
            .map_err(|e| format!("Failed to update config: {e}"))?;

        info!("[Bilibili/Broadcast] Started! Room: {long_room_id}");

        Ok(())
    }

    pub async fn stop(&self) {
        self.cancel.lock().unwrap().cancel();
    }
}
