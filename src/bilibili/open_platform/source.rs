use crate::bilibili::SourceStartResult;
use crate::bilibili::live_socket::{LiveSocketOptions, LiveSocketStatus, run_live_socket};
use crate::bilibili::open_platform::parser::parse_open_platform_gift;
use crate::bilibili::open_platform::signer::sign_open_platform_request;
use crate::config::types::GiftEvent;
use crate::config::{ConfigHandle, RuntimeStateStore};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

const BASE_URL: &str = "https://live-open.biliapi.com";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct OpenPlatformResponse {
    code: i64,
    #[serde(default)]
    message: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct StartData {
    game_info: GameInfo,
    websocket_info: WebsocketInfo,
}

#[derive(Debug, Deserialize)]
struct GameInfo {
    game_id: String,
}

#[derive(Debug, Deserialize)]
struct WebsocketInfo {
    wss_link: Vec<String>,
    auth_body: String,
}

#[derive(Debug, Deserialize)]
struct AuthBody {
    key: String,
    group: Option<String>,
    roomid: Option<u64>,
    protoover: Option<u64>,
    uid: Option<u64>,
}

pub struct OpenPlatformSource {
    config: ConfigHandle,
    state: RuntimeStateStore,
    gift_tx: mpsc::Sender<GiftEvent>,
    app_id: u64,
    game_id: Option<String>,
    cancel: CancellationToken,
    http: reqwest::Client,
}

impl OpenPlatformSource {
    pub fn new(
        config: ConfigHandle,
        state: RuntimeStateStore,
        gift_tx: mpsc::Sender<GiftEvent>,
    ) -> Self {
        Self {
            config,
            state,
            gift_tx,
            app_id: 0,
            game_id: None,
            cancel: CancellationToken::new(),
            http: reqwest::Client::new(),
        }
    }

    fn reset_cancel(&mut self) -> CancellationToken {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        self.cancel.clone()
    }

    pub async fn start(
        &mut self,
        app_key: Option<String>,
        app_secret: Option<String>,
        code: Option<String>,
        app_id: Option<u64>,
    ) -> Result<SourceStartResult, String> {
        let cancel = self.reset_cancel();

        let cfg = self.config.lock().await;
        let defaults = &cfg.get().bilibili.open_platform;
        let app_key = app_key.unwrap_or_else(|| defaults.app_key.clone());
        let app_secret = app_secret.unwrap_or_else(|| defaults.app_secret.clone());
        let code = code.unwrap_or_else(|| defaults.code.clone());
        let app_id = app_id.unwrap_or(defaults.app_id);
        drop(cfg);

        if app_key.is_empty() || app_secret.is_empty() || code.is_empty() || app_id == 0 {
            return Err("code, appId, appKey and appSecret required".into());
        }

        self.app_id = app_id;

        self.clear_stale_game(&app_key, &app_secret, app_id).await;

        let resp = self
            .request(
                &app_key,
                &app_secret,
                "/v2/app/start",
                &serde_json::json!({"code": code, "app_id": app_id}),
            )
            .await
            .map_err(|e| e.to_string())?;

        if resp.code == 7002 {
            return Err("直播间已有互动玩法会话，请先结束已有会话后重试".into());
        }
        if resp.code == 7001 {
            return Err("请求冷却期：上一个会话未正常结束，请稍后 (约 30-60s) 重试".into());
        }
        if resp.code != 0 {
            return Err(format!(
                "连接失败: {}",
                resp.message.unwrap_or_else(|| resp.code.to_string())
            ));
        }

        let data_val = resp.data.ok_or("missing data in response")?;
        let data: StartData =
            serde_json::from_value(data_val).map_err(|e| format!("parse start data: {e}"))?;

        self.handle_start_success(data, app_key, app_secret, code, app_id, cancel)
            .await
    }

    pub async fn stop(&mut self) {
        self.cancel.cancel();
        if let Some(game_id) = self.game_id.take() {
            // Use the credentials currently saved in config to end the session.
            let cfg = self.config.lock().await;
            let creds = &cfg.get().bilibili.open_platform;
            let app_key = creds.app_key.clone();
            let app_secret = creds.app_secret.clone();
            drop(cfg);
            self.end_game(&app_key, &app_secret, &game_id, self.app_id)
                .await;
        }
    }

    async fn request(
        &self,
        app_key: &str,
        app_secret: &str,
        path: &str,
        params: &serde_json::Value,
    ) -> Result<OpenPlatformResponse, reqwest::Error> {
        let headers = sign_open_platform_request(params, app_key, app_secret);

        let mut req = self.http.post(format!("{BASE_URL}{path}"));
        for (key, value) in headers {
            req = req.header(key, value);
        }
        req = req.json(params);

        info!("[Bilibili/OpenPlatform] POST {path}");
        let resp = req.send().await?;
        let data: OpenPlatformResponse = resp.json().await?;
        info!(
            "[Bilibili/OpenPlatform] Response {path}: code={}",
            data.code
        );
        Ok(data)
    }

    async fn clear_stale_game(&mut self, app_key: &str, app_secret: &str, app_id: u64) {
        let stale_id = self.state.open_platform_game_id().to_string();
        if stale_id.is_empty() {
            return;
        }
        info!("[Bilibili/OpenPlatform] Cleaning stale game: {stale_id}");
        self.end_game(app_key, app_secret, &stale_id, app_id).await;
    }

    async fn end_game(&mut self, app_key: &str, app_secret: &str, game_id: &str, app_id: u64) {
        match self
            .request(
                app_key,
                app_secret,
                "/v2/app/end",
                &serde_json::json!({"game_id": game_id, "app_id": app_id}),
            )
            .await
        {
            Ok(resp) if resp.code == 0 => {
                if let Err(e) = self.state.set_open_platform_game_id(String::new()).await {
                    error!("[Bilibili/OpenPlatform] Failed to clear state: {e}");
                }
            }
            Ok(resp) => {
                error!(
                    "[Bilibili/OpenPlatform] endGame failed: code={} message={:?}",
                    resp.code, resp.message
                );
            }
            Err(e) => {
                error!("[Bilibili/OpenPlatform] endGame error: {e}");
            }
        }
    }

    async fn handle_start_success(
        &mut self,
        data: StartData,
        app_key: String,
        app_secret: String,
        code: String,
        app_id: u64,
        cancel: CancellationToken,
    ) -> Result<SourceStartResult, String> {
        let auth: AuthBody = serde_json::from_str(&data.websocket_info.auth_body)
            .map_err(|e| format!("auth_body 格式错误: {e}"))?;

        self.game_id = Some(data.game_info.game_id.clone());

        if let Err(e) = self
            .state
            .set_open_platform_game_id(data.game_info.game_id.clone())
            .await
        {
            warn!("[Bilibili/OpenPlatform] Failed to save state: {e}");
        }

        if let Err(e) = self
            .config
            .lock()
            .await
            .update(serde_json::json!({
                "bilibili": {
                    "source": "open-platform",
                    "openPlatform": {
                        "appKey": app_key,
                        "appSecret": app_secret,
                        "code": code,
                        "appId": app_id
                    }
                }
            }))
            .await
        {
            warn!("[Bilibili/OpenPlatform] Failed to update config: {e}");
        }

        let game_id_for_hb = data.game_info.game_id.clone();
        let gift_tx = self.gift_tx.clone();
        let room_id = auth.roomid;

        let auth_value = serde_json::json!({
            "key": auth.key,
            "group": auth.group,
            "roomid": auth.roomid,
            "protoover": auth.protoover.unwrap_or(2),
            "uid": auth.uid.unwrap_or(0),
        });

        let (msg_tx, mut msg_rx) = mpsc::channel::<serde_json::Value>(256);
        let (inner_status_tx, inner_status_rx) = mpsc::channel::<LiveSocketStatus>(16);

        let ls_cancel = cancel.clone();
        tokio::spawn(async move {
            run_live_socket(
                LiveSocketOptions {
                    label: "Bilibili/OpenPlatform".into(),
                    urls: data.websocket_info.wss_link,
                    auth: auth_value,
                    room_id,
                    on_message: msg_tx,
                    on_status: inner_status_tx,
                },
                ls_cancel,
            )
            .await;
        });

        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                if let Some(gift) = parse_open_platform_gift(&msg) {
                    let _ = gift_tx.send(gift).await;
                }
            }
        });

        let hb_http = self.http.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(20));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {}
                }
                let params = serde_json::json!({"game_id": &game_id_for_hb});
                let headers = sign_open_platform_request(&params, &app_key, &app_secret);
                let mut req = hb_http.post(format!("{BASE_URL}/v2/app/heartbeat"));
                for (key, value) in headers {
                    req = req.header(key, value);
                }
                req = req.json(&params);
                if let Err(e) = req.send().await {
                    warn!("[Bilibili/OpenPlatform] Heartbeat error: {e}");
                }
            }
        });

        info!(
            "[Bilibili/OpenPlatform] Started! Game ID: {}, Room: {:?}",
            data.game_info.game_id, auth.roomid
        );

        Ok(SourceStartResult {
            status_rx: inner_status_rx,
            room_id: auth.roomid,
            game_id: Some(data.game_info.game_id),
        })
    }
}
