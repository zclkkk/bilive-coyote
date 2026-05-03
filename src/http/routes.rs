use tokio::sync::oneshot;

use crate::bilibili::{BilibiliCommand, BilibiliHandle, BilibiliStart};
use crate::config::{validate_bilibili_start, validate_manual_strength, ConfigHandle};
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

#[derive(Clone)]
pub struct AppState {
    pub config: ConfigHandle,
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
    let safety = state.config.snapshot().safety;
    let effective_limit_a = safety.limit_a.min(cs.limit_a);
    let effective_limit_b = safety.limit_b.min(cs.limit_b);

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
    let (tx, rx) = oneshot::channel();
    state
        .bilibili
        .cmd_tx
        .send(BilibiliCommand::Start(start, tx))
        .await
        .map_err(|_| ApiError::Internal("Bilibili manager has stopped".into()))?;

    rx.await
        .map_err(|_| ApiError::Internal("Bilibili manager did not respond".into()))?
        .map_err(ApiError::Validation)?;

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
    let safety = state.config.snapshot().safety;
    let effective_limit_a = safety.limit_a.min(cs.limit_a);
    let effective_limit_b = safety.limit_b.min(cs.limit_b);
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
    let cfg = state.config.snapshot();
    let value = serde_json::to_value(cfg).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(value))
}

pub async fn put_config(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let has_safety = body.get("safety").is_some();
    let has_rules = body.get("rules").is_some();
    {
        let mut cfg = state.config.lock().await;
        cfg.update(body).await?;

        if has_safety {
            let safety = &cfg.get().safety;
            let _ = state
                .strength_cmd
                .send(StrengthCommand::ConfigUpdate {
                    limit_a: safety.limit_a,
                    limit_b: safety.limit_b,
                    decay_enabled: safety.decay_enabled,
                    decay_rate: safety.decay_rate,
                })
                .await;
        }

        if has_rules {
            let rules = cfg.get().rules.clone();
            let _ = state
                .strength_cmd
                .send(StrengthCommand::RulesUpdate(rules))
                .await;
        }
    }

    Ok(Json(SuccessResponse { success: true }))
}

pub async fn get_rules(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let rules = &state.config.snapshot().rules;
    let value = serde_json::to_value(rules).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(value))
}

pub async fn put_rules(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let rules: Vec<crate::config::types::GiftRule> = serde_json::from_value(body)
        .map_err(|e| ApiError::Validation(format!("Invalid rules: {e}")))?;
    {
        let mut cfg = state.config.lock().await;
        cfg.set_rules(serde_json::to_value(&rules).unwrap()).await?;
    }
    let _ = state
        .strength_cmd
        .send(StrengthCommand::RulesUpdate(rules))
        .await;
    Ok(Json(SuccessResponse { success: true }))
}
