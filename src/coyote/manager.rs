use crate::config::types::Channel;
use crate::coyote::protocol::*;
use crate::coyote::session::CoyoteServerState;
use crate::coyote::waveform::{self, DEFAULT_WAVEFORM_ID};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformStatus {
    pub waveform_a: String,
    pub waveform_b: String,
}

impl Default for WaveformStatus {
    fn default() -> Self {
        Self {
            waveform_a: DEFAULT_WAVEFORM_ID.into(),
            waveform_b: DEFAULT_WAVEFORM_ID.into(),
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
    SelectWaveform {
        channel: Channel,
        waveform_id: String,
    },
    NextWaveform {
        channel: Channel,
    },
    EnsureWaveform {
        channel: Channel,
    },
    StopWaveform {
        channel: Channel,
    },
}

#[derive(Clone)]
pub struct CoyoteHandle {
    pub cmd_tx: mpsc::Sender<CoyoteCommand>,
    pub status: watch::Receiver<CoyoteStatus>,
    pub waveform_status: watch::Receiver<WaveformStatus>,
    pub feedback_tx: broadcast::Sender<CoyoteFeedback>,
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
    feedback_tx: broadcast::Sender<CoyoteFeedback>,
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
            return;
        }

        if let Some(feedback) = parse_feedback(message) {
            let _ = self.feedback_tx.send(feedback);
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
    waveforms: WaveformChannels,
    waveform_status_tx: watch::Sender<WaveformStatus>,
}

impl CoyoteManager {
    pub fn new() -> (Self, CoyoteHandle) {
        let bridge_id = uuid::Uuid::new_v4().to_string();
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (status_tx, status_rx) = watch::channel(CoyoteStatus::default());
        let (waveform_status_tx, waveform_status_rx) = watch::channel(WaveformStatus::default());
        let (feedback_tx, _) = broadcast::channel(32);

        let shared = Arc::new(CoyoteSharedState {
            paired: std::sync::Mutex::new(None),
        });

        let shared_handle = Arc::new(CoyoteSharedHandle {
            shared: shared.clone(),
            bridge_id: bridge_id.clone(),
            status_tx,
            feedback_tx: feedback_tx.clone(),
        });

        let server_state = Arc::new(CoyoteServerState {
            bridge_id: bridge_id.clone(),
            shared: shared_handle,
        });

        let manager = Self {
            bridge_id: bridge_id.clone(),
            cmd_rx,
            shared,
            waveforms: WaveformChannels::default(),
            waveform_status_tx,
        };

        let handle = CoyoteHandle {
            cmd_tx,
            status: status_rx,
            waveform_status: waveform_status_rx,
            feedback_tx,
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
                        Some(CoyoteCommand::SelectWaveform { channel, waveform_id }) => {
                            self.select_waveform(channel, waveform_id).await;
                        }
                        Some(CoyoteCommand::NextWaveform { channel }) => {
                            self.next_waveform(channel).await;
                        }
                        Some(CoyoteCommand::EnsureWaveform { channel }) => {
                            self.ensure_waveform(channel).await;
                        }
                        Some(CoyoteCommand::StopWaveform { channel }) => {
                            self.stop_waveform(channel).await;
                        }
                        None => {
                            self.abort_waveform(Channel::A);
                            self.abort_waveform(Channel::B);
                            break;
                        }
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

    async fn select_waveform(&mut self, channel: Channel, waveform_id: String) {
        if !waveform::is_waveform_id(&waveform_id) {
            return;
        }

        let was_running = {
            let state = self.waveforms.get_mut(channel);
            if state.selected == waveform_id {
                return;
            }
            state.selected = waveform_id;
            state.running.is_some()
        };

        if was_running {
            self.restart_waveform(channel).await;
        } else {
            self.emit_waveform_status();
        }
    }

    async fn next_waveform(&mut self, channel: Channel) {
        let next = waveform::next_waveform_id(&self.waveforms.get(channel).selected)
            .expect("selected waveform id is valid")
            .to_string();
        self.select_waveform(channel, next).await;
    }

    async fn ensure_waveform(&mut self, channel: Channel) {
        let should_start = {
            let state = self.waveforms.get(channel);
            state
                .running
                .as_ref()
                .is_none_or(|running| running.waveform_id != state.selected)
        };

        if should_start {
            self.restart_waveform(channel).await;
        }
    }

    async fn stop_waveform(&mut self, channel: Channel) {
        self.abort_waveform(channel);
        self.send_clear(channel).await;
        self.emit_waveform_status();
    }

    async fn restart_waveform(&mut self, channel: Channel) {
        let had_running = self.abort_waveform(channel);
        if had_running {
            self.send_clear(channel).await;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        self.start_waveform(channel);
        self.emit_waveform_status();
    }

    fn abort_waveform(&mut self, channel: Channel) -> bool {
        let state = self.waveforms.get_mut(channel);
        if let Some(running) = state.running.take() {
            running.handle.abort();
            true
        } else {
            false
        }
    }

    fn start_waveform(&mut self, channel: Channel) {
        let selected = self.waveforms.get(channel).selected.clone();
        let preset = waveform::find_waveform(&selected).expect("selected waveform id is valid");
        let command = preset.pulse_command(channel);
        let interval = preset.repeat_interval();
        let shared = self.shared.clone();
        let bridge_id = self.bridge_id.clone();
        let running_id = selected.clone();

        let handle = tokio::spawn(async move {
            loop {
                let Some((tx, app_id)) = snapshot_paired(&shared) else {
                    tokio::time::sleep(interval).await;
                    continue;
                };
                let msg = build_message("msg", &bridge_id, &app_id, &command);
                if tx.send(msg).await.is_err() {
                    tokio::time::sleep(interval).await;
                    continue;
                }
                tokio::time::sleep(interval).await;
            }
        });

        self.waveforms.get_mut(channel).running = Some(RunningWaveform {
            waveform_id: running_id,
            handle,
        });
    }

    async fn send_heartbeat(&self) {
        let Some((tx, app_id)) = self.snapshot_paired() else {
            return;
        };
        let msg = build_heartbeat(&app_id, &self.bridge_id);
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
        snapshot_paired(&self.shared)
    }

    fn emit_waveform_status(&self) {
        let _ = self.waveform_status_tx.send(WaveformStatus {
            waveform_a: self.waveforms.a.selected.clone(),
            waveform_b: self.waveforms.b.selected.clone(),
        });
    }
}

fn snapshot_paired(shared: &Arc<CoyoteSharedState>) -> Option<(mpsc::Sender<String>, String)> {
    shared
        .paired
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| (p.tx.clone(), p.client_id.clone()))
}

#[derive(Default)]
struct WaveformChannels {
    a: WaveformChannelState,
    b: WaveformChannelState,
}

impl WaveformChannels {
    fn get(&self, channel: Channel) -> &WaveformChannelState {
        match channel {
            Channel::A => &self.a,
            Channel::B => &self.b,
        }
    }

    fn get_mut(&mut self, channel: Channel) -> &mut WaveformChannelState {
        match channel {
            Channel::A => &mut self.a,
            Channel::B => &mut self.b,
        }
    }
}

struct WaveformChannelState {
    selected: String,
    running: Option<RunningWaveform>,
}

impl Default for WaveformChannelState {
    fn default() -> Self {
        Self {
            selected: DEFAULT_WAVEFORM_ID.into(),
            running: None,
        }
    }
}

struct RunningWaveform {
    waveform_id: String,
    handle: JoinHandle<()>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn heartbeat_uses_recipient_as_client_id() {
        let (manager, handle) = CoyoteManager::new();
        let (app_tx, mut app_rx) = mpsc::channel(1);
        let (close_tx, _) = watch::channel(false);

        handle
            .server_state
            .shared
            .register_app("app-id".into(), app_tx, close_tx);
        manager.send_heartbeat().await;

        let msg = app_rx.recv().await.unwrap();
        let parsed = parse_message(&msg).unwrap();
        assert_eq!(parsed.msg_type, "heartbeat");
        assert_eq!(parsed.client_id, "app-id");
        assert_eq!(parsed.target_id, manager.bridge_id);
        assert_eq!(parsed.message, ERR_SUCCESS);
    }
}
