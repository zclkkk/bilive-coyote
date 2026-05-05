use crate::config::types::Channel;
use serde::{Deserialize, Serialize};

pub const ERR_SUCCESS: &str = "200";
pub const ERR_PEER_DISCONNECTED: &str = "209";
pub const ERR_INVALID_QR_CLIENT_ID: &str = "210";
pub const ERR_NO_TARGET_ID: &str = "211";
pub const ERR_NOT_PAIRED: &str = "402";
pub const ERR_INVALID_JSON: &str = "403";
pub const ERR_MESSAGE_TOO_LONG: &str = "405";

pub const MAX_MESSAGE_LENGTH: usize = 1950;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoyoteMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub client_id: String,
    pub target_id: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub code: &'static str,
}

pub fn parse_message(data: &str) -> Result<CoyoteMessage, ParseError> {
    if data.len() > MAX_MESSAGE_LENGTH {
        return Err(ParseError {
            code: ERR_MESSAGE_TOO_LONG,
        });
    }

    let obj: serde_json::Value = serde_json::from_str(data).map_err(|_| ParseError {
        code: ERR_INVALID_JSON,
    })?;

    let map = obj.as_object().ok_or(ParseError {
        code: ERR_INVALID_JSON,
    })?;

    let msg_type = required_string(map, "type")?;
    let client_id = required_string(map, "clientId")?;
    let target_id = required_string(map, "targetId")?;
    let message = required_string(map, "message")?;

    Ok(CoyoteMessage {
        msg_type: msg_type.to_string(),
        client_id: client_id.to_string(),
        target_id: target_id.to_string(),
        message: message.to_string(),
    })
}

fn required_string<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<&'a str, ParseError> {
    map.get(field)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(ParseError {
            code: ERR_INVALID_JSON,
        })
}

pub fn build_message(msg_type: &str, client_id: &str, target_id: &str, message: &str) -> String {
    serde_json::json!({
        "type": msg_type,
        "clientId": client_id,
        "targetId": target_id,
        "message": message,
    })
    .to_string()
}

pub fn build_heartbeat(recipient_id: &str, paired_id: &str) -> String {
    build_message("heartbeat", recipient_id, paired_id, ERR_SUCCESS)
}

#[derive(Debug, Clone)]
pub struct StrengthFeedback {
    pub a: u8,
    pub b: u8,
    pub limit_a: u8,
    pub limit_b: u8,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoyoteFeedback {
    pub channel: Channel,
    pub button: u8,
    pub raw_index: u8,
}

pub fn parse_strength_feedback(message: &str) -> Option<StrengthFeedback> {
    let rest = message.strip_prefix("strength-")?;
    let mut parts = rest.split('+');
    let a = parse_u8_part(parts.next()?)?;
    let b = parse_u8_part(parts.next()?)?;
    let limit_a = parse_u8_part(parts.next()?)?;
    let limit_b = parse_u8_part(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(StrengthFeedback {
        a,
        b,
        limit_a,
        limit_b,
    })
}

pub fn parse_feedback(message: &str) -> Option<CoyoteFeedback> {
    let rest = message.strip_prefix("feedback-")?;
    let bytes = rest.as_bytes();
    if bytes.len() != 1 || !bytes[0].is_ascii_digit() {
        return None;
    }

    let raw_index = bytes[0] - b'0';
    let channel = if raw_index < 5 {
        Channel::A
    } else {
        Channel::B
    };
    Some(CoyoteFeedback {
        channel,
        button: raw_index % 5,
        raw_index,
    })
}

fn parse_u8_part(value: &str) -> Option<u8> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_valid() {
        let data = r#"{"type":"bind","clientId":"abc","targetId":"def","message":"hello"}"#;
        let msg = parse_message(data).unwrap();
        assert_eq!(msg.client_id, "abc");
        assert_eq!(msg.target_id, "def");
        assert_eq!(msg.message, "hello");
    }

    #[test]
    fn test_parse_message_too_long() {
        let long_data = format!(
            r#"{{"type":"msg","clientId":"a","targetId":"b","message":"{}"}}"#,
            "x".repeat(2000)
        );
        assert!(parse_message(&long_data).is_err());
    }

    #[test]
    fn test_parse_message_invalid_json() {
        assert!(parse_message("not json").is_err());
    }

    #[test]
    fn test_parse_message_missing_fields() {
        let data = r#"{"type":"bind"}"#;
        let result = parse_message(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_message_rejects_non_string_fields() {
        let data = r#"{"type":1,"clientId":"abc","targetId":"def","message":"hello"}"#;
        assert!(parse_message(data).is_err());

        let data = r#"{"type":"msg","clientId":"abc","targetId":"def","message":200}"#;
        assert!(parse_message(data).is_err());
    }

    #[test]
    fn test_build_message() {
        let msg = build_message("bind", "abc", "def", "200");
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed["type"], "bind");
        assert_eq!(parsed["clientId"], "abc");
        assert_eq!(parsed["targetId"], "def");
        assert_eq!(parsed["message"], "200");
    }

    #[test]
    fn test_build_heartbeat() {
        let msg = build_heartbeat("app", "bridge");
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed["type"], "heartbeat");
        assert_eq!(parsed["clientId"], "app");
        assert_eq!(parsed["targetId"], "bridge");
        assert_eq!(parsed["message"], ERR_SUCCESS);
    }

    #[test]
    fn test_parse_strength_feedback() {
        let fb = parse_strength_feedback("strength-10+20+80+90").unwrap();
        assert_eq!(fb.a, 10);
        assert_eq!(fb.b, 20);
        assert_eq!(fb.limit_a, 80);
        assert_eq!(fb.limit_b, 90);
    }

    #[test]
    fn test_parse_strength_feedback_invalid() {
        assert!(parse_strength_feedback("invalid").is_none());
        assert!(parse_strength_feedback("strength-10+20").is_none());
        assert!(parse_strength_feedback("strength-+10+20+80+90").is_none());
        assert!(parse_strength_feedback("strength-10+20+80+256").is_none());
    }

    #[test]
    fn test_parse_feedback() {
        let a0 = parse_feedback("feedback-0").unwrap();
        assert_eq!(a0.channel, Channel::A);
        assert_eq!(a0.button, 0);
        assert_eq!(a0.raw_index, 0);

        let a4 = parse_feedback("feedback-4").unwrap();
        assert_eq!(a4.channel, Channel::A);
        assert_eq!(a4.button, 4);
        assert_eq!(a4.raw_index, 4);

        let b0 = parse_feedback("feedback-5").unwrap();
        assert_eq!(b0.channel, Channel::B);
        assert_eq!(b0.button, 0);
        assert_eq!(b0.raw_index, 5);

        let b4 = parse_feedback("feedback-9").unwrap();
        assert_eq!(b4.channel, Channel::B);
        assert_eq!(b4.button, 4);
        assert_eq!(b4.raw_index, 9);
    }

    #[test]
    fn test_parse_feedback_invalid() {
        assert!(parse_feedback("feedback-").is_none());
        assert!(parse_feedback("feedback-10").is_none());
        assert!(parse_feedback("feedback-x").is_none());
        assert!(parse_feedback(" feedback-1").is_none());
        assert!(parse_feedback("feedback-1 ").is_none());
        assert!(parse_feedback("xfeedback-1").is_none());
    }
}
