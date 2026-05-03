use crate::bilibili::{BilibiliHandle, BilibiliManager};
use crate::config::types::GiftEvent;
use crate::config::{ConfigHandle, ConfigStore, RuntimeStateStore};
use crate::coyote::{CoyoteHandle, CoyoteManager};
use crate::engine::types::PanelEvent;
use crate::engine::{StrengthCommand, StrengthEngine, StrengthHandle};
use crate::http;
use crate::http::routes::AppState;
use crate::panel::PanelHub;
use axum::routing::get;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub struct App {
    config: ConfigHandle,
    bilibili_handle: BilibiliHandle,
    coyote_handle: CoyoteHandle,
    strength_handle: StrengthHandle,
    panel_hub: Arc<PanelHub>,
}

impl App {
    pub async fn init(config_path: &str, state_path: &str) -> anyhow::Result<Self> {
        let config = ConfigStore::load_or_default(config_path)
            .await
            .map_err(|e| anyhow::anyhow!("Config error: {e}"))?;
        let state = RuntimeStateStore::load_or_default(state_path);

        let config = ConfigHandle::new(config);
        let state = Arc::new(Mutex::new(state));

        let (gift_tx, gift_rx) = tokio::sync::mpsc::channel::<GiftEvent>(256);

        let (bilibili_manager, bilibili_handle) =
            BilibiliManager::new(config.clone(), state.clone(), gift_tx.clone());

        let (coyote_manager, coyote_handle) = CoyoteManager::new();

        let cfg_snapshot = config.snapshot();

        let (strength_engine, strength_handle) = StrengthEngine::new(
            cfg_snapshot.rules.clone(),
            (cfg_snapshot.safety.limit_a, cfg_snapshot.safety.limit_b),
            cfg_snapshot.safety.decay_enabled,
            cfg_snapshot.safety.decay_rate,
            coyote_handle.cmd_tx.clone(),
        );

        let panel_hub = Arc::new(PanelHub::new());

        let app = Self {
            config,
            bilibili_handle,
            coyote_handle,
            strength_handle,
            panel_hub,
        };

        let strength_cmd = app.strength_handle.cmd_tx.clone();
        tokio::spawn(async move {
            let mut rx = gift_rx;
            while let Some(gift) = rx.recv().await {
                let _ = strength_cmd.send(StrengthCommand::Gift(gift)).await;
            }
        });

        let coyote_status_rx = app.coyote_handle.status.clone();
        let strength_cmd_for_coyote = app.strength_handle.cmd_tx.clone();
        let panel_tx_for_coyote = app.panel_hub.tx.clone();
        let config_for_coyote = app.config.clone();
        tokio::spawn(async move {
            let mut rx = coyote_status_rx;
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                let status = rx.borrow().clone();
                if status.paired {
                    let _ = strength_cmd_for_coyote
                        .send(StrengthCommand::CoyoteFeedback {
                            strength_a: status.strength_a,
                            strength_b: status.strength_b,
                            limit_a: status.limit_a,
                            limit_b: status.limit_b,
                        })
                        .await;

                    let safety = config_for_coyote.snapshot().safety;
                    let effective_a = safety.limit_a.min(status.limit_a);
                    let effective_b = safety.limit_b.min(status.limit_b);

                    let event = PanelEvent {
                        event_type: "coyote:status".into(),
                        data: serde_json::json!({
                            "paired": status.paired,
                            "strengthA": status.strength_a,
                            "strengthB": status.strength_b,
                            "limitA": status.limit_a,
                            "limitB": status.limit_b,
                            "effectiveLimitA": effective_a,
                            "effectiveLimitB": effective_b,
                        }),
                    };
                    let _ = panel_tx_for_coyote.send(event);
                } else {
                    let _ = strength_cmd_for_coyote
                        .send(StrengthCommand::CoyoteDisconnected)
                        .await;

                    let safety = config_for_coyote.snapshot().safety;
                    let effective_a = safety.limit_a;
                    let effective_b = safety.limit_b;

                    let event = PanelEvent {
                        event_type: "coyote:status".into(),
                        data: serde_json::json!({
                            "paired": false,
                            "strengthA": 0,
                            "strengthB": 0,
                            "limitA": 200,
                            "limitB": 200,
                            "effectiveLimitA": effective_a,
                            "effectiveLimitB": effective_b,
                        }),
                    };
                    let _ = panel_tx_for_coyote.send(event);
                }
            }
        });

        let bilibili_status_rx = app.bilibili_handle.status.clone();
        let panel_tx_for_bilibili = app.panel_hub.tx.clone();
        tokio::spawn(async move {
            let mut rx = bilibili_status_rx;
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                let status = rx.borrow().clone();
                let event = PanelEvent {
                    event_type: "bilibili:status".into(),
                    data: serde_json::to_value(&status).unwrap_or_default(),
                };
                let _ = panel_tx_for_bilibili.send(event);
            }
        });

        let panel_tx_for_strength = app.panel_hub.tx.clone();
        let mut strength_panel_rx = app.strength_handle.panel_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match strength_panel_rx.recv().await {
                    Ok(event) => {
                        let _ = panel_tx_for_strength.send(event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Panel broadcast lagged, skipped {n} events");
                    }
                    Err(_) => break,
                }
            }
        });

        tokio::spawn(bilibili_manager.run());
        tokio::spawn(coyote_manager.run());
        tokio::spawn(strength_engine.run());

        Ok(app)
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let cfg_snapshot = self.config.snapshot();

        let bridge_id = self.coyote_handle.bridge_id.clone();
        let coyote_server_state = self.coyote_handle.server_state.clone();

        let app_state = AppState {
            config: self.config.clone(),
            bilibili: self.bilibili_handle.clone(),
            coyote: self.coyote_handle.clone(),
            strength_cmd: self.strength_handle.cmd_tx.clone(),
            strength_status: self.strength_handle.status.clone(),
            panel: self.panel_hub.clone(),
        };

        let main_app = http::create_router(app_state);

        let http_addr = format!(
            "{}:{}",
            cfg_snapshot.server.host, cfg_snapshot.server.http_port
        );
        let listener = tokio::net::TcpListener::bind(&http_addr).await?;
        info!("[Server] HTTP + WS started on http://{http_addr}");

        let coyote_app = axum::Router::new()
            .route(
                &format!("/{bridge_id}"),
                get(crate::coyote::session::coyote_ws_handler),
            )
            .with_state(coyote_server_state);

        let coyote_addr = format!(
            "{}:{}",
            cfg_snapshot.server.host, cfg_snapshot.coyote.ws_port
        );
        let coyote_listener = tokio::net::TcpListener::bind(&coyote_addr).await?;
        info!(
            "[Coyote] WS server started on port {}",
            cfg_snapshot.coyote.ws_port
        );
        info!("[Coyote] Bridge ID: {bridge_id}");

        let display_host = if cfg_snapshot.server.host == "0.0.0.0" {
            "localhost"
        } else {
            &cfg_snapshot.server.host
        };
        info!(
            "[Bilive-Coyote] Ready! Open http://{display_host}:{}",
            cfg_snapshot.server.http_port
        );

        tokio::select! {
            r = axum::serve(listener, main_app) => r?,
            r = axum::serve(coyote_listener, coyote_app) => r?,
        }

        Ok(())
    }
}
