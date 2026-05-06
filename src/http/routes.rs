use tokio::sync::oneshot;

use crate::bilibili::{BilibiliCommand, BilibiliHandle, BilibiliStart};
use crate::config::types::Channel;
use crate::config::{ConfigHandle, parse_bilibili_start, parse_manual_strength};
use crate::coyote::waveform;
use crate::coyote::{CoyoteCommand, CoyoteHandle, generate_qr_data_url};
use crate::engine::StrengthCommand;
use crate::engine::types::{PanelEvent, StrengthStatus};
use crate::http::error::ApiError;
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub config: ConfigHandle,
    pub bilibili: BilibiliHandle,
    pub coyote: CoyoteHandle,
    pub strength_cmd: tokio::sync::mpsc::Sender<StrengthCommand>,
    pub strength_status: tokio::sync::watch::Receiver<StrengthStatus>,
    pub panel_tx: broadcast::Sender<PanelEvent>,
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
    let default_source = state.config.snapshot().bilibili.source;
    let input = parse_bilibili_start(body, default_source)?;
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
        .send(BilibiliCommand::Stop(None))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(SuccessResponse { success: true }))
}

pub async fn bilibili_status(
    State(state): State<AppState>,
) -> Json<crate::engine::types::BilibiliStatus> {
    Json(state.bilibili.status.borrow().clone())
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
    let host = resolve_qrcode_host(&config.server.host)?;
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

fn resolve_qrcode_host(configured_host: &str) -> Result<String, ApiError> {
    resolve_qrcode_host_with(configured_host, || {
        local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .map_err(|e| e.to_string())
    })
}

fn resolve_qrcode_host_with<F, E>(configured_host: &str, local_ip: F) -> Result<String, ApiError>
where
    F: FnOnce() -> Result<String, E>,
    E: std::fmt::Display,
{
    if configured_host == "0.0.0.0" {
        local_ip().map_err(|e| {
            ApiError::Internal(format!(
                "Cannot determine local IP; set server.host explicitly: {e}"
            ))
        })
    } else {
        Ok(configured_host.to_string())
    }
}

pub async fn coyote_strength(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let ms = parse_manual_strength(body)?;
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

pub async fn coyote_waveforms(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let status = state.coyote.waveform_status.borrow().clone();
    Ok(Json(serde_json::json!({
        "items": waveform::list_waveforms(),
        "selectedA": status.waveform_a,
        "selectedB": status.waveform_b,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformCommandInput {
    pub action: String,
    pub channel: String,
    #[serde(default)]
    pub waveform_id: Option<String>,
}

pub async fn coyote_waveform(
    State(state): State<AppState>,
    Json(body): Json<WaveformCommandInput>,
) -> Result<Json<SuccessResponse>, ApiError> {
    let channels = parse_waveform_channels(&body.channel)?;
    match body.action.as_str() {
        "select" => {
            let waveform_id = body
                .waveform_id
                .filter(|id| waveform::is_waveform_id(id))
                .ok_or_else(|| ApiError::Validation("Unknown waveformId".into()))?;
            for channel in channels {
                state
                    .coyote
                    .cmd_tx
                    .send(CoyoteCommand::SelectWaveform {
                        channel,
                        waveform_id: waveform_id.clone(),
                    })
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
        "next" => {
            for channel in channels {
                state
                    .coyote
                    .cmd_tx
                    .send(CoyoteCommand::NextWaveform { channel })
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
        _ => return Err(ApiError::Validation("Unknown waveform action".into())),
    }

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

fn parse_waveform_channels(value: &str) -> Result<Vec<Channel>, ApiError> {
    match value {
        "A" => Ok(vec![Channel::A]),
        "B" => Ok(vec![Channel::B]),
        "both" => Ok(vec![Channel::A, Channel::B]),
        _ => Err(ApiError::Validation("Invalid channel".into())),
    }
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
    reject_runtime_only_config(&body)?;

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

fn reject_runtime_only_config(body: &Value) -> Result<(), ApiError> {
    if body.get("server").is_some() || body.get("coyote").is_some() {
        return Err(ApiError::Validation(
            "server/coyote config changes require restart".into(),
        ));
    }
    Ok(())
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
        cfg.set_rules(rules.clone()).await?;
    }
    let _ = state
        .strength_cmd
        .send(StrengthCommand::RulesUpdate(rules))
        .await;
    Ok(Json(SuccessResponse { success: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qrcode_host_uses_explicit_host() {
        let host = resolve_qrcode_host_with("192.0.2.1", || -> Result<String, &'static str> {
            panic!("local ip should not be used")
        })
        .unwrap();
        assert_eq!(host, "192.0.2.1");
    }

    #[test]
    fn qrcode_host_rejects_missing_local_ip() {
        let err = resolve_qrcode_host_with("0.0.0.0", || -> Result<String, &'static str> {
            Err("missing")
        })
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Cannot determine local IP; set server.host explicitly: missing"
        );
    }

    #[test]
    fn config_rejects_runtime_only_sections() {
        let err = reject_runtime_only_config(&serde_json::json!({
            "coyote": { "wsPort": 10000 }
        }))
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "server/coyote config changes require restart"
        );

        assert!(
            reject_runtime_only_config(&serde_json::json!({
                "safety": { "limitA": 100 }
            }))
            .is_ok()
        );
    }
}
