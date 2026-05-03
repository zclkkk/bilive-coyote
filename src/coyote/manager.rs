use crate::config::types::Channel;
use crate::coyote::protocol::*;
use crate::coyote::session::CoyoteServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoyoteStatus {
    pub paired: bool,
    pub strength_a: u8,
    pub strength_b: u8,
    pub limit_a: u8,
    pub limit_b: u8,
}

impl Default for CoyoteStatus {
    fn default() -> Self {
        Self {
            paired: false,
            strength_a: 0,
            strength_b: 0,
            limit_a: 200,
            limit_b: 200,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CoyoteCommand {
    SendStrength {
        channel: Channel,
        mode: u8,
        value: u8,
    },
    Clear {
        channel: Channel,
    },
}

#[derive(Clone)]
pub struct CoyoteHandle {
    pub cmd_tx: mpsc::Sender<CoyoteCommand>,
    pub status: watch::Receiver<CoyoteStatus>,
    pub bridge_id: String,
    pub server_state: Arc<CoyoteServerState>,
}

struct PairedApp {
    client_id: String,
    tx: mpsc::Sender<String>,
    close_tx: watch::Sender<bool>,
}

struct CoyoteSharedState {
    paired: std::sync::Mutex<Option<PairedApp>>,
}

pub struct CoyoteSharedHandle {
    shared: Arc<CoyoteSharedState>,
    bridge_id: String,
    status_tx: watch::Sender<CoyoteStatus>,
}

impl CoyoteSharedHandle {
    pub fn register_app(
        &self,
        client_id: String,
        tx: mpsc::Sender<String>,
        close_tx: watch::Sender<bool>,
    ) -> Option<mpsc::Sender<String>> {
        let new = PairedApp {
            client_id,
            tx,
            close_tx,
        };
        let old = self.shared.paired.lock().unwrap().replace(new);
        if let Some(prev) = old.as_ref() {
            let _ = prev.close_tx.send(true);
        }
        let _ = self.status_tx.send(CoyoteStatus {
            paired: true,
            ..CoyoteStatus::default()
        });
        old.map(|p| p.tx)
    }

    pub fn is_paired_app(&self, client_id: &str) -> bool {
        self.shared
            .paired
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|p| p.client_id == client_id)
    }

    pub fn bridge_id(&self) -> &str {
        &self.bridge_id
    }

    pub fn handle_app_message(&self, message: &str) {
        if let Some(fb) = parse_strength_feedback(message) {
            let status = CoyoteStatus {
                paired: true,
                strength_a: fb.a,
                strength_b: fb.b,
                limit_a: fb.limit_a,
                limit_b: fb.limit_b,
            };
            let _ = self.status_tx.send(status);
        }
    }

    pub fn disconnect_app(&self, client_id: &str) {
        let mut guard = self.shared.paired.lock().unwrap();
        if guard.as_ref().is_some_and(|p| p.client_id == client_id) {
            *guard = None;
            drop(guard);
            let _ = self.status_tx.send(CoyoteStatus::default());
        }
    }
}

pub struct CoyoteManager {
    bridge_id: String,
    cmd_rx: mpsc::Receiver<CoyoteCommand>,
    shared: Arc<CoyoteSharedState>,
}

impl CoyoteManager {
    pub fn new() -> (Self, CoyoteHandle) {
        let bridge_id = uuid::Uuid::new_v4().to_string();
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (status_tx, status_rx) = watch::channel(CoyoteStatus::default());

        let shared = Arc::new(CoyoteSharedState {
            paired: std::sync::Mutex::new(None),
        });

        let shared_handle = Arc::new(CoyoteSharedHandle {
            shared: shared.clone(),
            bridge_id: bridge_id.clone(),
            status_tx,
        });

        let server_state = Arc::new(CoyoteServerState {
            bridge_id: bridge_id.clone(),
            shared: shared_handle,
        });

        let manager = Self {
            bridge_id: bridge_id.clone(),
            cmd_rx,
            shared,
        };

        let handle = CoyoteHandle {
            cmd_tx,
            status: status_rx,
            bridge_id,
            server_state,
        };

        (manager, handle)
    }

    pub async fn run(mut self) {
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
        heartbeat.tick().await;

        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(CoyoteCommand::SendStrength { channel, mode, value }) => {
                            self.send_strength(channel, mode, value).await;
                        }
                        Some(CoyoteCommand::Clear { channel }) => {
                            self.send_clear(channel).await;
                        }
                        None => break,
                    }
                }
                _ = heartbeat.tick() => {
                    self.send_heartbeat().await;
                }
            }
        }
    }

    async fn send_strength(&self, channel: Channel, mode: u8, value: u8) {
        let ch_num = match channel {
            Channel::A => 1,
            Channel::B => 2,
        };
        self.send_app_command(&format!("strength-{ch_num}+{mode}+{value}"))
            .await;
    }

    async fn send_clear(&self, channel: Channel) {
        let ch_num = match channel {
            Channel::A => 1,
            Channel::B => 2,
        };
        self.send_app_command(&format!("clear-{ch_num}")).await;
    }

    async fn send_heartbeat(&self) {
        let Some((tx, app_id)) = self.snapshot_paired() else {
            return;
        };
        let msg = build_message("heartbeat", &app_id, &self.bridge_id, ERR_SUCCESS);
        let _ = tx.send(msg).await;
    }

    async fn send_app_command(&self, command: &str) {
        let Some((tx, app_id)) = self.snapshot_paired() else {
            return;
        };
        let msg = build_message("msg", &self.bridge_id, &app_id, command);
        let _ = tx.send(msg).await;
    }

    fn snapshot_paired(&self) -> Option<(mpsc::Sender<String>, String)> {
        self.shared
            .paired
            .lock()
            .unwrap()
            .as_ref()
            .map(|p| (p.tx.clone(), p.client_id.clone()))
    }
}
