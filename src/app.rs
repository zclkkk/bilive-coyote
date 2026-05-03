use crate::bilibili::{BilibiliHandle, BilibiliManager};
use crate::config::types::GiftEvent;
use crate::config::{ConfigHandle, ConfigStore, RuntimeStateStore};
use crate::coyote::{CoyoteHandle, CoyoteManager};
use crate::engine::types::PanelEvent;
use crate::engine::{StrengthEngine, StrengthHandle};
use crate::http;
use crate::http::routes::AppState;
use axum::routing::get;
use tokio::sync::broadcast;
use tracing::info;

pub struct App {
    config: ConfigHandle,
    bilibili_handle: BilibiliHandle,
    coyote_handle: CoyoteHandle,
    strength_handle: StrengthHandle,
    panel_tx: broadcast::Sender<PanelEvent>,
}

impl App {
    pub async fn init(config_path: &str, state_path: &str) -> anyhow::Result<Self> {
        let config = ConfigStore::load_or_default(config_path)
            .await
            .map_err(|e| anyhow::anyhow!("Config error: {e}"))?;
        let state = RuntimeStateStore::load_or_default(state_path);

        let config = ConfigHandle::new(config);
        let cfg_snapshot = config.snapshot();

        let (panel_tx, _) = broadcast::channel::<PanelEvent>(256);
        let (gift_tx, gift_rx) = tokio::sync::mpsc::channel::<GiftEvent>(256);

        let (bilibili_manager, bilibili_handle) =
            BilibiliManager::new(config.clone(), state, gift_tx, panel_tx.clone());

        let (coyote_manager, coyote_handle) = CoyoteManager::new();

        let (strength_engine, strength_handle) = StrengthEngine::new(
            cfg_snapshot.rules.clone(),
            cfg_snapshot.safety.limit_a,
            cfg_snapshot.safety.limit_b,
            cfg_snapshot.safety.decay_enabled,
            cfg_snapshot.safety.decay_rate,
            gift_rx,
            coyote_handle.status.clone(),
            coyote_handle.cmd_tx.clone(),
            panel_tx.clone(),
        );

        tokio::spawn(bilibili_manager.run());
        tokio::spawn(coyote_manager.run());
        tokio::spawn(strength_engine.run());

        Ok(Self {
            config,
            bilibili_handle,
            coyote_handle,
            strength_handle,
            panel_tx,
        })
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
            panel_tx: self.panel_tx.clone(),
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
