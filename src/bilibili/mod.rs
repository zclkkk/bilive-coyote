pub mod broadcast;
mod http_client;
pub mod live_socket;
pub mod open_platform;

use crate::bilibili::broadcast::BroadcastSource;
use crate::bilibili::live_socket::LiveSocketStatus;
use crate::bilibili::open_platform::OpenPlatformSource;
use crate::config::types::{BilibiliSourceType, GiftEvent};
use crate::config::{BilibiliStartInput, ConfigHandle, RuntimeStateStore};
use crate::engine::types::{BilibiliStatus, PanelEvent};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast as bcast;
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BilibiliStart {
    OpenPlatform {
        #[serde(default)]
        app_key: Option<String>,
        #[serde(default)]
        app_secret: Option<String>,
        #[serde(default)]
        code: Option<String>,
        #[serde(default)]
        app_id: Option<u64>,
    },
    Broadcast {
        #[serde(default)]
        room_id: Option<u64>,
        #[serde(default)]
        login_json: Option<String>,
    },
}

impl From<BilibiliStartInput> for BilibiliStart {
    fn from(input: BilibiliStartInput) -> Self {
        match input.source {
            BilibiliSourceType::OpenPlatform => BilibiliStart::OpenPlatform {
                app_key: input.app_key,
                app_secret: input.app_secret,
                code: input.code,
                app_id: input.app_id,
            },
            BilibiliSourceType::Broadcast => BilibiliStart::Broadcast {
                room_id: input.room_id,
                login_json: input.login_json,
            },
        }
    }
}

pub struct SourceStartResult {
    pub status_rx: mpsc::Receiver<LiveSocketStatus>,
    pub room_id: Option<u64>,
    pub game_id: Option<String>,
}

#[derive(Debug)]
pub enum BilibiliCommand {
    Start(BilibiliStart, oneshot::Sender<Result<(), String>>),
    Stop(Option<oneshot::Sender<()>>),
}

#[derive(Clone)]
pub struct BilibiliHandle {
    pub cmd_tx: mpsc::Sender<BilibiliCommand>,
    pub status: watch::Receiver<BilibiliStatus>,
}

pub struct BilibiliManager {
    cmd_rx: mpsc::Receiver<BilibiliCommand>,
    status_tx: watch::Sender<BilibiliStatus>,
    panel_tx: bcast::Sender<PanelEvent>,
    current_status: BilibiliStatus,
    open_platform: OpenPlatformSource,
    broadcast: BroadcastSource,
    live_status_rx: Option<mpsc::Receiver<LiveSocketStatus>>,
}

impl BilibiliManager {
    pub fn new(
        config: ConfigHandle,
        state: RuntimeStateStore,
        gift_tx: mpsc::Sender<GiftEvent>,
        panel_tx: bcast::Sender<PanelEvent>,
    ) -> (Self, BilibiliHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let initial_status = BilibiliStatus {
            source: config.snapshot().bilibili.source,
            connected: false,
            room_id: None,
            game_id: None,
            error: None,
        };
        let (status_tx, status_rx) = watch::channel(initial_status.clone());

        let open_platform = OpenPlatformSource::new(config.clone(), state, gift_tx.clone());
        let broadcast = BroadcastSource::new(config.clone(), gift_tx);

        let manager = Self {
            cmd_rx,
            status_tx,
            panel_tx,
            current_status: initial_status,
            open_platform,
            broadcast,
            live_status_rx: None,
        };

        let handle = BilibiliHandle {
            cmd_tx,
            status: status_rx,
        };

        (manager, handle)
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(BilibiliCommand::Start(start, reply)) => {
                            self.handle_start(start, reply).await;
                        }
                        Some(BilibiliCommand::Stop(reply)) => {
                            self.handle_stop().await;
                            if let Some(reply) = reply {
                                let _ = reply.send(());
                            }
                        }
                        None => break,
                    }
                }
                status = async {
                    match self.live_status_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(s) = status {
                        self.current_status.connected = s.connected;
                        self.current_status.error = s.error;
                        if s.room_id.is_some() {
                            self.current_status.room_id = s.room_id;
                        }
                        self.publish_status();
                    } else {
                        self.live_status_rx = None;
                    }
                }
            }
        }
    }

    async fn handle_start(
        &mut self,
        start: BilibiliStart,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        self.handle_stop().await;

        let result = match start {
            BilibiliStart::OpenPlatform {
                app_key,
                app_secret,
                code,
                app_id,
            } => {
                self.current_status = BilibiliStatus {
                    source: BilibiliSourceType::OpenPlatform,
                    connected: false,
                    room_id: None,
                    game_id: None,
                    error: None,
                };
                self.open_platform
                    .start(app_key, app_secret, code, app_id)
                    .await
            }
            BilibiliStart::Broadcast {
                room_id,
                login_json,
            } => {
                self.current_status = BilibiliStatus {
                    source: BilibiliSourceType::Broadcast,
                    connected: false,
                    room_id: None,
                    game_id: None,
                    error: None,
                };
                self.broadcast.start(room_id, login_json).await
            }
        };

        match result {
            Ok(source_result) => {
                self.live_status_rx = Some(source_result.status_rx);
                self.current_status.room_id = source_result.room_id;
                self.current_status.game_id = source_result.game_id;
                self.publish_status();
                let _ = reply.send(Ok(()));
            }
            Err(e) => {
                self.current_status.error = Some(e.clone());
                self.publish_status();
                let _ = reply.send(Err(e));
            }
        }
    }

    fn publish_status(&self) {
        let _ = self.status_tx.send(self.current_status.clone());
        let event = PanelEvent {
            event_type: "bilibili:status".into(),
            data: serde_json::to_value(&self.current_status).expect("BilibiliStatus serializes"),
        };
        let _ = self.panel_tx.send(event);
    }

    async fn handle_stop(&mut self) {
        match self.current_status.source {
            BilibiliSourceType::OpenPlatform => {
                self.open_platform.stop().await;
            }
            BilibiliSourceType::Broadcast => {
                self.broadcast.stop();
            }
        }
        self.live_status_rx = None;
        self.current_status = BilibiliStatus {
            source: self.current_status.source,
            connected: false,
            room_id: None,
            game_id: None,
            error: None,
        };
        self.publish_status();
    }
}
