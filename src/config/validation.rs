use crate::config::types::{AppConfig, BilibiliSourceType, Channel, GiftRule};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}

impl From<serde_json::Error> for ValidationError {
    fn from(e: serde_json::Error) -> Self {
        ValidationError::Message(e.to_string())
    }
}

fn err(msg: impl Into<String>) -> ValidationError {
    ValidationError::Message(msg.into())
}

pub fn validate_app_config(cfg: &AppConfig) -> Result<(), ValidationError> {
    if cfg.coyote.ws_port == 0 {
        return Err(err("coyote.wsPort must be between 1 and 65535"));
    }
    if cfg.server.http_port == 0 {
        return Err(err("server.httpPort must be between 1 and 65535"));
    }
    if cfg.server.host.trim().is_empty() {
        return Err(err("server.host is required"));
    }
    if cfg.safety.limit_a > 200 {
        return Err(err("safety.limitA must be between 0 and 200"));
    }
    if cfg.safety.limit_b > 200 {
        return Err(err("safety.limitB must be between 0 and 200"));
    }
    if cfg.safety.decay_rate < 1 || cfg.safety.decay_rate > 200 {
        return Err(err("safety.decayRate must be between 1 and 200"));
    }
    validate_rules(&cfg.rules)?;
    Ok(())
}

pub fn validate_rules(rules: &[GiftRule]) -> Result<(), ValidationError> {
    for (i, rule) in rules.iter().enumerate() {
        validate_rule(rule, i)?;
    }
    Ok(())
}

fn validate_rule(rule: &GiftRule, index: usize) -> Result<(), ValidationError> {
    if rule.gift_name.trim().is_empty() {
        return Err(err(format!("rules[{index}].giftName is required")));
    }
    if rule.strength_add < 1 || rule.strength_add > 200 {
        return Err(err(format!(
            "rules[{index}].strengthAdd must be between 1 and 200"
        )));
    }
    if rule.duration < 1 {
        return Err(err(format!("rules[{index}].duration must be >= 1")));
    }
    if let Some(gift_id) = rule.gift_id {
        if gift_id < 1 {
            return Err(err(format!("rules[{index}].giftId must be >= 1")));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualStrength {
    pub channel: Channel,
    pub value: u8,
}

pub fn parse_manual_strength(value: serde_json::Value) -> Result<ManualStrength, ValidationError> {
    let ms: ManualStrength = serde_json::from_value(value)?;
    if ms.value > 200 {
        return Err(err("value must be between 0 and 200"));
    }
    Ok(ms)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BilibiliStartInput {
    pub source: BilibiliSourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BilibiliStartWire {
    #[serde(default)]
    source: Option<BilibiliSourceType>,
    #[serde(default)]
    app_key: Option<String>,
    #[serde(default)]
    app_secret: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    app_id: Option<u64>,
    #[serde(default)]
    room_id: Option<u64>,
}

pub fn parse_bilibili_start(
    value: serde_json::Value,
    default_source: BilibiliSourceType,
) -> Result<BilibiliStartInput, ValidationError> {
    let wire: BilibiliStartWire = serde_json::from_value(value)?;
    if matches!(wire.app_id, Some(0)) {
        return Err(err("appId must be >= 1"));
    }
    if matches!(wire.room_id, Some(0)) {
        return Err(err("roomId must be >= 1"));
    }
    Ok(BilibiliStartInput {
        source: wire.source.unwrap_or(default_source),
        app_key: wire.app_key,
        app_secret: wire.app_secret,
        code: wire.code,
        app_id: wire.app_id,
        room_id: wire.room_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        validate_app_config(&AppConfig::default()).unwrap();
    }

    #[test]
    fn rejects_safety_limit_overflow() {
        let mut cfg = AppConfig::default();
        cfg.safety.limit_a = 201;
        assert!(validate_app_config(&cfg).is_err());
    }

    #[test]
    fn rejects_zero_decay_rate() {
        let mut cfg = AppConfig::default();
        cfg.safety.decay_rate = 0;
        assert!(validate_app_config(&cfg).is_err());
    }

    #[test]
    fn rejects_blank_host() {
        let mut cfg = AppConfig::default();
        cfg.server.host = "  ".into();
        assert!(validate_app_config(&cfg).is_err());
    }

    #[test]
    fn manual_strength_clamps_at_200() {
        let body = serde_json::json!({"channel": "A", "value": 201});
        assert!(parse_manual_strength(body).is_err());
    }

    #[test]
    fn manual_strength_accepts_valid() {
        let body = serde_json::json!({"channel": "B", "value": 50});
        let ms = parse_manual_strength(body).unwrap();
        assert!(matches!(ms.channel, Channel::B));
        assert_eq!(ms.value, 50);
    }

    #[test]
    fn bilibili_start_falls_back_to_default_source() {
        let body = serde_json::json!({"roomId": 123});
        let parsed = parse_bilibili_start(body, BilibiliSourceType::Broadcast).unwrap();
        assert_eq!(parsed.source, BilibiliSourceType::Broadcast);
        assert_eq!(parsed.room_id, Some(123));
    }

    #[test]
    fn bilibili_start_rejects_zero_room_id() {
        let body = serde_json::json!({"source": "broadcast", "roomId": 0});
        assert!(parse_bilibili_start(body, BilibiliSourceType::Broadcast).is_err());
    }
}
