use crate::bilibili::SourceStartResult;
use crate::bilibili::broadcast::parser::parse_broadcast_gift;
use crate::bilibili::broadcast::wbi::fetch_danmu_auth_info;
use crate::bilibili::http_client::HyperHttpClient;
use crate::bilibili::live_socket::{LiveSocketOptions, LiveSocketStatus, run_live_socket};
use crate::config::ConfigHandle;
use crate::config::types::GiftEvent;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub struct BroadcastSource {
    config: ConfigHandle,
    gift_tx: mpsc::Sender<GiftEvent>,
    cancel: CancellationToken,
    http: HyperHttpClient,
}

impl BroadcastSource {
    pub fn new(config: ConfigHandle, gift_tx: mpsc::Sender<GiftEvent>) -> Self {
        Self {
            config,
            gift_tx,
            cancel: CancellationToken::new(),
            http: HyperHttpClient::new(),
        }
    }

    fn reset_cancel(&mut self) -> CancellationToken {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        self.cancel.clone()
    }

    pub async fn start(
        &mut self,
        room_id: Option<u64>,
        login_json: Option<String>,
    ) -> Result<SourceStartResult, String> {
        let cancel = self.reset_cancel();

        let cfg = self.config.lock().await;
        let requested = room_id.unwrap_or(cfg.get().bilibili.broadcast.room_id);
        drop(cfg);

        if requested == 0 {
            return Err("roomId required".into());
        }

        let danmu = fetch_danmu_auth_info(&self.http, requested, login_json).await?;

        let gift_tx = self.gift_tx.clone();

        let auth = serde_json::json!({
            "uid": danmu.uid.unwrap_or(0),
            "roomid": danmu.room_id,
            "protover": 3,
            "platform": "web",
            "type": 2,
            "key": danmu.key,
            "buvid": danmu.buvid3,
        });

        let (msg_tx, mut msg_rx) = mpsc::channel::<serde_json::Value>(256);
        let (inner_status_tx, inner_status_rx) = mpsc::channel::<LiveSocketStatus>(16);

        let ls_cancel = cancel.clone();
        tokio::spawn(async move {
            run_live_socket(
                LiveSocketOptions {
                    label: "Bilibili/Broadcast".into(),
                    urls: danmu.urls,
                    auth,
                    room_id: Some(danmu.room_id),
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

        if let Err(e) = self
            .config
            .lock()
            .await
            .update(serde_json::json!({
                "bilibili": {
                    "source": "broadcast",
                    "broadcast": {
                        "roomId": danmu.room_id
                    }
                }
            }))
            .await
        {
            warn!("[Bilibili/Broadcast] Failed to update config: {e}");
        }

        info!("[Bilibili/Broadcast] Started! Room: {}", danmu.room_id);

        Ok(SourceStartResult {
            status_rx: inner_status_rx,
            room_id: Some(danmu.room_id),
            game_id: None,
        })
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }
}
