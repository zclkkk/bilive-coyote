use crate::config::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrengthStatus {
    pub a: u8,
    pub b: u8,
    pub app_limit_a: u8,
    pub app_limit_b: u8,
    pub effective_limit_a: u8,
    pub effective_limit_b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrengthChangeEvent {
    pub channel: Channel,
    pub delta: i16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute: Option<u8>,
    pub source: StrengthSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gift_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrengthSource {
    Gift,
    Manual,
    Decay,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftLogEvent {
    pub gift_id: u64,
    pub gift_name: String,
    pub coin_type: String,
    pub total_coin: u64,
    pub num: u32,
    pub uid: u64,
    pub uname: String,
    pub timestamp: u64,
    pub strength_delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BilibiliStatus {
    pub source: BilibiliSourceType,
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
}
