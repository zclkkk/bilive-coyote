pub mod gift_mapper;
pub mod types;

use crate::config::types::{Channel, GiftEvent, GiftRule};
use crate::coyote::{CoyoteCommand, CoyoteStatus};
use crate::engine::gift_mapper::{apply_rule, build_gift_log, match_rule};
use crate::engine::types::{PanelEvent, StrengthSource, StrengthStatus};
use tokio::sync::{broadcast, mpsc, watch};

#[derive(Debug, Clone, Default)]
struct ChannelPair<T> {
    a: T,
    b: T,
}

impl<T> ChannelPair<T> {
    fn get(&self, ch: Channel) -> &T {
        match ch {
            Channel::A => &self.a,
            Channel::B => &self.b,
        }
    }

    fn get_mut(&mut self, ch: Channel) -> &mut T {
        match ch {
            Channel::A => &mut self.a,
            Channel::B => &mut self.b,
        }
    }
}

impl<T: Copy> ChannelPair<T> {
    fn splat(value: T) -> Self {
        Self { a: value, b: value }
    }
}

#[derive(Debug, Clone)]
pub enum StrengthCommand {
    ManualStrength {
        channel: Channel,
        value: u8,
    },
    EmergencyStop,
    ConfigUpdate {
        limit_a: u8,
        limit_b: u8,
        decay_enabled: bool,
        decay_rate: u8,
    },
    RulesUpdate(Vec<GiftRule>),
}

#[derive(Debug, Clone, Default)]
struct StrengthEntry {
    value: u8,
    baseline: u8,
    expiries: Vec<Expiry>,
}

#[derive(Debug, Clone)]
struct Expiry {
    until: std::time::Instant,
    delta: u8,
}

#[derive(Clone)]
pub struct StrengthHandle {
    pub cmd_tx: mpsc::Sender<StrengthCommand>,
    pub status: watch::Receiver<StrengthStatus>,
}

pub struct StrengthEngine {
    channels: ChannelPair<StrengthEntry>,
    app_limits: ChannelPair<u8>,
    config_limits: ChannelPair<u8>,
    decay_enabled: bool,
    decay_rate: u8,
    rules: Vec<GiftRule>,
    cmd_rx: mpsc::Receiver<StrengthCommand>,
    gift_rx: mpsc::Receiver<GiftEvent>,
    coyote_status_rx: watch::Receiver<CoyoteStatus>,
    status_tx: watch::Sender<StrengthStatus>,
    panel_tx: broadcast::Sender<PanelEvent>,
    coyote_cmd_tx: mpsc::Sender<CoyoteCommand>,
}

impl StrengthEngine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rules: Vec<GiftRule>,
        limit_a: u8,
        limit_b: u8,
        decay_enabled: bool,
        decay_rate: u8,
        gift_rx: mpsc::Receiver<GiftEvent>,
        coyote_status_rx: watch::Receiver<CoyoteStatus>,
        coyote_cmd_tx: mpsc::Sender<CoyoteCommand>,
        panel_tx: broadcast::Sender<PanelEvent>,
    ) -> (Self, StrengthHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let config_limits = ChannelPair {
            a: limit_a,
            b: limit_b,
        };
        let initial = StrengthStatus {
            a: 0,
            b: 0,
            app_limit_a: 200,
            app_limit_b: 200,
            effective_limit_a: limit_a,
            effective_limit_b: limit_b,
        };
        let (status_tx, status_rx) = watch::channel(initial);

        let engine = Self {
            channels: ChannelPair::default(),
            app_limits: ChannelPair::splat(200),
            config_limits,
            decay_enabled,
            decay_rate,
            rules,
            cmd_rx,
            gift_rx,
            coyote_status_rx,
            status_tx,
            panel_tx,
            coyote_cmd_tx,
        };

        let handle = StrengthHandle {
            cmd_tx,
            status: status_rx,
        };

        (engine, handle)
    }

    pub async fn run(mut self) {
        let mut decay_tick = tokio::time::interval(std::time::Duration::from_secs(1));
        decay_tick.tick().await;

        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(StrengthCommand::ManualStrength { channel, value }) => {
                            self.handle_manual(channel, value).await;
                        }
                        Some(StrengthCommand::EmergencyStop) => {
                            self.handle_emergency().await;
                        }
                        Some(StrengthCommand::ConfigUpdate { limit_a, limit_b, decay_enabled, decay_rate }) => {
                            self.config_limits = ChannelPair { a: limit_a, b: limit_b };
                            self.decay_enabled = decay_enabled;
                            self.decay_rate = decay_rate;
                            self.enforce_limits().await;
                        }
                        Some(StrengthCommand::RulesUpdate(rules)) => {
                            self.rules = rules;
                        }
                        None => break,
                    }
                }
                gift = self.gift_rx.recv() => {
                    let Some(gift) = gift else { break };
                    self.handle_gift(gift).await;
                }
                changed = self.coyote_status_rx.changed() => {
                    if changed.is_err() { break; }
                    let status = self.coyote_status_rx.borrow_and_update().clone();
                    self.handle_coyote_status(status).await;
                }
                _ = decay_tick.tick() => {
                    self.run_decay().await;
                }
            }
        }
    }

    fn effective_limit(&self, channel: Channel) -> u8 {
        (*self.config_limits.get(channel)).min(*self.app_limits.get(channel))
    }

    async fn handle_gift(&mut self, gift: GiftEvent) {
        let rule = self.rules.iter().find(|r| match_rule(r, &gift));
        let strength_delta = if let Some(rule) = rule {
            let (events, delta_str) = apply_rule(rule, &gift);
            for event in events {
                let ch = event.channel;
                let limit = self.effective_limit(ch);
                let entry = self.channels.get_mut(ch);
                let new_value = (entry.value as i32)
                    .saturating_add(event.delta)
                    .clamp(0, limit as i32) as u8;
                let actual_delta = new_value - entry.value;
                if actual_delta > 0 {
                    entry.value = new_value;
                    entry.expiries.push(Expiry {
                        until: std::time::Instant::now()
                            + std::time::Duration::from_secs(event.duration.unwrap_or(0)),
                        delta: actual_delta,
                    });
                    self.send_coyote_strength(ch, new_value).await;
                    self.emit_strength_event(ch, new_value, StrengthSource::Gift)
                        .await;
                }
            }
            delta_str
        } else {
            "—".into()
        };

        let log = build_gift_log(&gift, strength_delta);
        let _ = self.panel_tx.send(PanelEvent {
            event_type: "gift".into(),
            data: serde_json::to_value(&log).expect("GiftLogEvent serializes"),
        });
    }

    async fn handle_manual(&mut self, channel: Channel, value: u8) {
        let limit = self.effective_limit(channel);
        let value = value.min(limit);
        let entry = self.channels.get_mut(channel);
        entry.value = value;
        entry.baseline = value;
        entry.expiries.clear();
        self.send_coyote_strength(channel, value).await;
        self.emit_strength_event(channel, value, StrengthSource::Manual)
            .await;
    }

    async fn handle_emergency(&mut self) {
        for ch in [Channel::A, Channel::B] {
            let entry = self.channels.get_mut(ch);
            entry.value = 0;
            entry.baseline = 0;
            entry.expiries.clear();
            let _ = self
                .coyote_cmd_tx
                .send(CoyoteCommand::SendStrength {
                    channel: ch,
                    mode: 2,
                    value: 0,
                })
                .await;
            let _ = self
                .coyote_cmd_tx
                .send(CoyoteCommand::Clear { channel: ch })
                .await;
            self.emit_strength_event(ch, 0, StrengthSource::Emergency)
                .await;
        }
    }

    async fn handle_coyote_status(&mut self, status: CoyoteStatus) {
        if status.paired {
            self.app_limits = ChannelPair {
                a: status.limit_a,
                b: status.limit_b,
            };
            self.apply_channel_feedback(Channel::A, status.strength_a)
                .await;
            self.apply_channel_feedback(Channel::B, status.strength_b)
                .await;
        } else {
            for ch in [Channel::A, Channel::B] {
                let entry = self.channels.get_mut(ch);
                entry.value = 0;
                entry.baseline = 0;
                entry.expiries.clear();
            }
            self.app_limits = ChannelPair::splat(200);
        }
        self.emit_status();
        self.emit_coyote_status(&status);
    }

    async fn apply_channel_feedback(&mut self, channel: Channel, app_value: u8) {
        let limit = self.effective_limit(channel);
        let value = app_value.min(limit);
        let entry = self.channels.get_mut(channel);

        if entry.value != value {
            entry.value = value;
            entry.baseline = value;
            entry.expiries.clear();
        }

        if app_value != value {
            self.send_coyote_strength(channel, value).await;
        }
    }

    async fn run_decay(&mut self) {
        if !self.decay_enabled {
            return;
        }

        let now = std::time::Instant::now();
        for ch in [Channel::A, Channel::B] {
            let new_value = {
                let entry = self.channels.get_mut(ch);
                entry.expiries.retain(|exp| exp.until > now);
                let active_delta: u8 = entry.expiries.iter().map(|exp| exp.delta).sum();
                let floor = (entry.baseline as u16 + active_delta as u16).min(255) as u8;
                if entry.value <= floor {
                    continue;
                }
                let decay_delta = (entry.value - floor).min(self.decay_rate);
                if decay_delta == 0 {
                    continue;
                }
                entry.value -= decay_delta;
                entry.value
            };
            self.send_coyote_strength(ch, new_value).await;
            self.emit_strength_event(ch, new_value, StrengthSource::Decay)
                .await;
        }
    }

    async fn enforce_limits(&mut self) {
        for ch in [Channel::A, Channel::B] {
            let limit = self.effective_limit(ch);
            let entry = self.channels.get_mut(ch);
            if entry.value > limit {
                entry.value = limit;
                entry.baseline = limit;
                entry.expiries.clear();
                self.send_coyote_strength(ch, limit).await;
            }
        }
        self.emit_status();
    }

    async fn send_coyote_strength(&self, channel: Channel, value: u8) {
        let _ = self
            .coyote_cmd_tx
            .send(CoyoteCommand::SendStrength {
                channel,
                mode: 2,
                value,
            })
            .await;
    }

    async fn emit_strength_event(&self, channel: Channel, value: u8, source: StrengthSource) {
        self.emit_status();
        let _ = self.panel_tx.send(PanelEvent {
            event_type: "strength".into(),
            data: serde_json::json!({
                "channel": format!("{channel:?}"),
                "value": value,
                "source": source,
            }),
        });
    }

    fn emit_coyote_status(&self, status: &CoyoteStatus) {
        let event = PanelEvent {
            event_type: "coyote:status".into(),
            data: serde_json::json!({
                "paired": status.paired,
                "strengthA": status.strength_a,
                "strengthB": status.strength_b,
                "limitA": status.limit_a,
                "limitB": status.limit_b,
                "effectiveLimitA": self.effective_limit(Channel::A),
                "effectiveLimitB": self.effective_limit(Channel::B),
            }),
        };
        let _ = self.panel_tx.send(event);
    }

    fn emit_status(&self) {
        let status = StrengthStatus {
            a: self.channels.get(Channel::A).value,
            b: self.channels.get(Channel::B).value,
            app_limit_a: self.app_limits.a,
            app_limit_b: self.app_limits.b,
            effective_limit_a: self.effective_limit(Channel::A),
            effective_limit_b: self.effective_limit(Channel::B),
        };
        let _ = self.status_tx.send(status);
    }
}
