use crate::bilibili::{BilibiliCommand, BilibiliHandle, BilibiliStart};
use crate::config::{validate_bilibili_start, validate_manual_strength, ConfigStore};
use crate::coyote::{generate_qr_data_url, CoyoteHandle};
use crate::engine::types::StrengthStatus;
use crate::engine::StrengthCommand;
use crate::http::error::ApiError;
use crate::panel::PanelHub;
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<ConfigStore>>,
    pub bilibili: BilibiliHandle,
    pub coyote: CoyoteHandle,
    pub strength_cmd: tokio::sync::mpsc::Sender<StrengthCommand>,
    pub strength_status: tokio::sync::watch::Receiver<StrengthStatus>,
    pub panel: Arc<PanelHub>,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
}

pub async fn get_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let bilibili_status = state.bilibili.status.borrow().clone();
    let cs = state.coyote.status.borrow().clone();
    let strength_status = state.strength_status.borrow().clone();
    let cfg = state.config.lock().await;
    let effective_limit_a = cfg.get().safety.limit_a.min(cs.limit_a);
    let effective_limit_b = cfg.get().safety.limit_b.min(cs.limit_b);
    drop(cfg);

    Ok(Json(serde_json::json!({
        "bilibili": bilibili_status,
        "coyote": {
            "paired": cs.paired,
            "strengthA": cs.strength_a,
            "strengthB": cs.strength_b,
            "limitA": cs.limit_a,
            "limitB": cs.limit_b,
            "effectiveLimitA": effective_limit_a,
            "effectiveLimitB": effective_limit_b,
        },
        "strength": strength_status,
    })))
}

pub async fn bilibili_start(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let default_source = {
        let cfg = state.config.lock().await;
        cfg.get().bilibili.source
    };
    let input = validate_bilibili_start(&body, default_source)?;
    let start = BilibiliStart::from(input);
    state
        .bilibili
        .cmd_tx
        .send(BilibiliCommand::Start(start))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(SuccessResponse { success: true }))
}

pub async fn bilibili_stop(
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse>, ApiError> {
    state
        .bilibili
        .cmd_tx
        .send(BilibiliCommand::Stop)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(SuccessResponse { success: true }))
}

pub async fn bilibili_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let status = state.bilibili.status.borrow().clone();
    Ok(Json(serde_json::to_value(&status).unwrap_or_default()))
}

pub async fn coyote_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let cs = state.coyote.status.borrow().clone();
    let cfg = state.config.lock().await;
    let effective_limit_a = cfg.get().safety.limit_a.min(cs.limit_a);
    let effective_limit_b = cfg.get().safety.limit_b.min(cs.limit_b);
    Ok(Json(serde_json::json!({
        "paired": cs.paired,
        "strengthA": cs.strength_a,
        "strengthB": cs.strength_b,
        "limitA": cs.limit_a,
        "limitB": cs.limit_b,
        "effectiveLimitA": effective_limit_a,
        "effectiveLimitB": effective_limit_b,
    })))
}

pub async fn coyote_qrcode(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let cfg = state.config.lock().await;
    let config = cfg.get();
    let host = if config.server.host == "0.0.0.0" {
        local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".into())
    } else {
        config.server.host.clone()
    };
    let ws_port = config.coyote.ws_port;
    drop(cfg);

    let bridge_id = state.coyote.bridge_id.clone();
    let ws_url = format!("ws://{host}:{ws_port}/{bridge_id}");
    let qr_content = format!("https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#{ws_url}");

    match generate_qr_data_url(&qr_content) {
        Ok(qr) => Ok(Json(serde_json::json!({ "qrcode": qr }))),
        Err(_) => Err(ApiError::NotFound("QR code unavailable".into())),
    }
}

pub async fn coyote_strength(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let ms = validate_manual_strength(&body)?;
    state
        .strength_cmd
        .send(StrengthCommand::ManualStrength {
            channel: ms.channel,
            value: ms.value,
        })
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(SuccessResponse { success: true }))
}

pub async fn coyote_emergency(
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse>, ApiError> {
    state
        .strength_cmd
        .send(StrengthCommand::EmergencyStop)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(SuccessResponse { success: true }))
}

pub async fn get_config(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let cfg = state.config.lock().await;
    let value = serde_json::to_value(cfg.get()).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(value))
}

pub async fn put_config(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let safety_update = body.get("safety").cloned();
    {
        let mut cfg = state.config.lock().await;
        cfg.update(body).await?;
    }

    if let Some(safety) = safety_update {
        let limit_a = safety.get("limitA").and_then(|v| v.as_u64()).unwrap_or(80) as u8;
        let limit_b = safety.get("limitB").and_then(|v| v.as_u64()).unwrap_or(80) as u8;
        let decay_enabled = safety
            .get("decayEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let decay_rate = safety
            .get("decayRate")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as u8;

        let _ = state
            .strength_cmd
            .send(StrengthCommand::ConfigUpdate {
                limit_a,
                limit_b,
                decay_enabled,
                decay_rate,
            })
            .await;
    }

    Ok(Json(SuccessResponse { success: true }))
}

pub async fn get_rules(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let cfg = state.config.lock().await;
    let rules = &cfg.get().rules;
    let value = serde_json::to_value(rules).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(value))
}

pub async fn put_rules(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    {
        let mut cfg = state.config.lock().await;
        cfg.set_rules(body).await?;
    }
    Ok(Json(SuccessResponse { success: true }))
}
