use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoinType {
    Gold,
    Silver,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleChannel {
    A,
    B,
    #[serde(rename = "both", alias = "Both")]
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    A,
    B,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftRule {
    pub gift_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gift_id: Option<u64>,
    pub coin_type: CoinType,
    pub channel: RuleChannel,
    pub strength_add: u8,
    pub duration: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waveform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftEvent {
    pub gift_id: u64,
    pub gift_name: String,
    pub coin_type: String,
    pub total_coin: u64,
    pub num: u32,
    pub uid: u64,
    pub uname: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BilibiliSourceType {
    OpenPlatform,
    Broadcast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPlatformConfig {
    #[serde(default)]
    pub app_key: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub app_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastConfig {
    #[serde(default)]
    pub room_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BilibiliConfig {
    pub source: BilibiliSourceType,
    #[serde(rename = "openPlatform")]
    pub open_platform: OpenPlatformConfig,
    pub broadcast: BroadcastConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoyoteConfig {
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,
}

fn default_ws_port() -> u16 {
    9999
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default = "default_host")]
    pub host: String,
}

fn default_http_port() -> u16 {
    3000
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyConfig {
    #[serde(default = "default_limit")]
    pub limit_a: u8,
    #[serde(default = "default_limit")]
    pub limit_b: u8,
    #[serde(default = "default_true")]
    pub decay_enabled: bool,
    #[serde(default = "default_decay_rate")]
    pub decay_rate: u8,
}

fn default_limit() -> u8 {
    80
}

fn default_true() -> bool {
    true
}

fn default_decay_rate() -> u8 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub bilibili: BilibiliConfig,
    pub coyote: CoyoteConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub rules: Vec<GiftRule>,
    pub safety: SafetyConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bilibili: BilibiliConfig {
                source: BilibiliSourceType::OpenPlatform,
                open_platform: OpenPlatformConfig {
                    app_key: String::new(),
                    app_secret: String::new(),
                    code: String::new(),
                    app_id: 0,
                },
                broadcast: BroadcastConfig { room_id: 0 },
            },
            coyote: CoyoteConfig {
                ws_port: default_ws_port(),
            },
            server: ServerConfig {
                http_port: default_http_port(),
                host: default_host(),
            },
            rules: vec![
                GiftRule {
                    gift_name: "小心心".to_string(),
                    gift_id: None,
                    coin_type: CoinType::Silver,
                    channel: RuleChannel::A,
                    strength_add: 5,
                    duration: 10,
                    waveform: None,
                },
                GiftRule {
                    gift_name: "辣条".to_string(),
                    gift_id: None,
                    coin_type: CoinType::Silver,
                    channel: RuleChannel::B,
                    strength_add: 3,
                    duration: 5,
                    waveform: None,
                },
            ],
            safety: SafetyConfig {
                limit_a: default_limit(),
                limit_b: default_limit(),
                decay_enabled: default_true(),
                decay_rate: default_decay_rate(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct RuntimeState {
    #[serde(default)]
    pub open_platform_game_id: String,
}
