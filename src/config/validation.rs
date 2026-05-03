use crate::config::types::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}

pub fn validate_config(value: &serde_json::Value) -> Result<AppConfig, ValidationError> {
    let obj = value
        .as_object()
        .ok_or_else(|| ValidationError::Message("config must be an object".into()))?;

    let bilibili = validate_bilibili(opt_get(obj, "bilibili", "bilibili")?)?;
    let coyote = validate_coyote(opt_get(obj, "coyote", "coyote")?)?;
    let server = validate_server(opt_get(obj, "server", "server")?)?;
    let rules = validate_rules(opt_get(obj, "rules", "rules")?)?;
    let safety = validate_safety(opt_get(obj, "safety", "safety")?)?;

    Ok(AppConfig {
        bilibili,
        coyote,
        server,
        rules,
        safety,
    })
}

fn opt_get<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    name: &str,
) -> Result<&'a serde_json::Value, ValidationError> {
    obj.get(key)
        .ok_or_else(|| ValidationError::Message(format!("{name} is required")))
}

fn validate_bilibili(value: &serde_json::Value) -> Result<BilibiliConfig, ValidationError> {
    let obj = require_object(value, "bilibili")?;

    let source_str = obj.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let source = match source_str {
        "open-platform" => BilibiliSourceType::OpenPlatform,
        "broadcast" => BilibiliSourceType::Broadcast,
        _ => {
            return Err(ValidationError::Message(
                "bilibili.source is invalid".into(),
            ))
        }
    };

    let op = opt_get(obj, "openPlatform", "bilibili.openPlatform")?;
    let op_obj = require_object(op, "bilibili.openPlatform")?;

    let bc = opt_get(obj, "broadcast", "bilibili.broadcast")?;
    let bc_obj = require_object(bc, "bilibili.broadcast")?;

    Ok(BilibiliConfig {
        source,
        open_platform: OpenPlatformConfig {
            app_key: op_obj
                .get("appKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            app_secret: op_obj
                .get("appSecret")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            code: op_obj
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            app_id: op_obj.get("appId").and_then(|v| v.as_u64()).unwrap_or(0),
        },
        broadcast: BroadcastConfig {
            room_id: bc_obj.get("roomId").and_then(|v| v.as_u64()).unwrap_or(0),
        },
    })
}

fn validate_coyote(value: &serde_json::Value) -> Result<CoyoteConfig, ValidationError> {
    let obj = require_object(value, "coyote")?;
    let ws_port = require_port(obj.get("wsPort"), "coyote.wsPort")?;
    Ok(CoyoteConfig { ws_port })
}

fn validate_server(value: &serde_json::Value) -> Result<ServerConfig, ValidationError> {
    let obj = require_object(value, "server")?;
    let http_port = require_port(obj.get("httpPort"), "server.httpPort")?;
    let host = require_non_empty_string(obj.get("host"), "server.host")?;
    Ok(ServerConfig { http_port, host })
}

fn validate_safety(value: &serde_json::Value) -> Result<SafetyConfig, ValidationError> {
    let obj = require_object(value, "safety")?;
    Ok(SafetyConfig {
        limit_a: require_u8_range(obj.get("limitA"), "safety.limitA", 0, 200)?,
        limit_b: require_u8_range(obj.get("limitB"), "safety.limitB", 0, 200)?,
        decay_enabled: require_bool(obj.get("decayEnabled"), "safety.decayEnabled")?,
        decay_rate: require_u8_range(obj.get("decayRate"), "safety.decayRate", 1, 200)?,
    })
}

pub fn validate_rules(value: &serde_json::Value) -> Result<Vec<GiftRule>, ValidationError> {
    let arr = value
        .as_array()
        .ok_or_else(|| ValidationError::Message("rules must be an array".into()))?;
    arr.iter()
        .enumerate()
        .map(|(i, v)| validate_rule(v, &format!("rules[{i}]")))
        .collect()
}

fn validate_rule(value: &serde_json::Value, name: &str) -> Result<GiftRule, ValidationError> {
    let obj = require_object(value, name)?;
    let gift_name = require_non_empty_string(obj.get("giftName"), &format!("{name}.giftName"))?;

    let coin_type_str = obj
        .get("coinType")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::Message(format!("{name}.coinType must be a string")))?;
    let coin_type = match coin_type_str {
        "gold" => CoinType::Gold,
        "silver" => CoinType::Silver,
        "all" => CoinType::All,
        _ => {
            return Err(ValidationError::Message(format!(
                "{name}.coinType is invalid"
            )))
        }
    };

    let channel_str = obj
        .get("channel")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::Message(format!("{name}.channel must be a string")))?;
    let channel = match channel_str {
        "A" => RuleChannel::A,
        "B" => RuleChannel::B,
        "both" => RuleChannel::Both,
        _ => {
            return Err(ValidationError::Message(format!(
                "{name}.channel is invalid"
            )))
        }
    };

    let gift_id = obj.get("giftId").and_then(|v| v.as_u64());

    Ok(GiftRule {
        gift_name,
        gift_id,
        coin_type,
        channel,
        strength_add: require_u8_range(
            obj.get("strengthAdd"),
            &format!("{name}.strengthAdd"),
            1,
            200,
        )?,
        duration: require_u64_min(obj.get("duration"), &format!("{name}.duration"), 1)?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualStrength {
    pub channel: Channel,
    pub value: u8,
}

pub fn validate_manual_strength(
    value: &serde_json::Value,
) -> Result<ManualStrength, ValidationError> {
    let obj = require_object(value, "body")?;
    let channel_str = obj
        .get("channel")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::Message("channel must be a string".into()))?;
    let channel = match channel_str {
        "A" => Channel::A,
        "B" => Channel::B,
        _ => return Err(ValidationError::Message("channel is invalid".into())),
    };
    Ok(ManualStrength {
        channel,
        value: require_u8_range(obj.get("value"), "value", 0, 200)?,
    })
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

pub fn validate_bilibili_start(
    value: &serde_json::Value,
    default_source: BilibiliSourceType,
) -> Result<BilibiliStartInput, ValidationError> {
    let obj = require_object(value, "body")?;
    let source = match obj
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
    {
        "open-platform" => BilibiliSourceType::OpenPlatform,
        "broadcast" => BilibiliSourceType::Broadcast,
        "" => default_source,
        _ => return Err(ValidationError::Message("source is invalid".into())),
    };

    Ok(BilibiliStartInput {
        source,
        app_key: obj.get("appKey").and_then(|v| v.as_str()).map(String::from),
        app_secret: obj
            .get("appSecret")
            .and_then(|v| v.as_str())
            .map(String::from),
        code: obj.get("code").and_then(|v| v.as_str()).map(String::from),
        app_id: obj.get("appId").and_then(|v| v.as_u64()),
        room_id: obj.get("roomId").and_then(|v| v.as_u64()),
    })
}

fn require_object<'a>(
    value: &'a serde_json::Value,
    name: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, ValidationError> {
    value
        .as_object()
        .ok_or_else(|| ValidationError::Message(format!("{name} must be an object")))
}

fn require_string<'a>(
    value: Option<&'a serde_json::Value>,
    name: &str,
) -> Result<&'a str, ValidationError> {
    value
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::Message(format!("{name} must be a string")))
}

fn require_non_empty_string(
    value: Option<&serde_json::Value>,
    name: &str,
) -> Result<String, ValidationError> {
    let s = require_string(value, name)?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::Message(format!("{name} is required")));
    }
    Ok(trimmed.to_string())
}

fn require_bool(value: Option<&serde_json::Value>, name: &str) -> Result<bool, ValidationError> {
    value
        .and_then(|v| v.as_bool())
        .ok_or_else(|| ValidationError::Message(format!("{name} must be a boolean")))
}

fn require_port(value: Option<&serde_json::Value>, name: &str) -> Result<u16, ValidationError> {
    let n = value
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ValidationError::Message(format!("{name} must be an integer")))?;
    if !(1..=65535).contains(&n) {
        return Err(ValidationError::Message(format!(
            "{name} must be between 1 and 65535"
        )));
    }
    Ok(n as u16)
}

fn require_u8_range(
    value: Option<&serde_json::Value>,
    name: &str,
    min: u8,
    max: u8,
) -> Result<u8, ValidationError> {
    let n = value
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ValidationError::Message(format!("{name} must be an integer")))?;
    if n < min as u64 || n > max as u64 {
        return Err(ValidationError::Message(format!(
            "{name} must be between {min} and {max}"
        )));
    }
    Ok(n as u8)
}

fn require_u64_min(
    value: Option<&serde_json::Value>,
    name: &str,
    min: u64,
) -> Result<u64, ValidationError> {
    let n = value
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ValidationError::Message(format!("{name} must be an integer")))?;
    if n < min {
        return Err(ValidationError::Message(format!("{name} must be >= {min}")));
    }
    Ok(n)
}
