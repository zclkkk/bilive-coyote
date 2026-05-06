use md5::{Digest, Md5};
use serde::de::DeserializeOwned;

use crate::bilibili::http_client::{HttpError, HyperHttpClient, empty_body, uri};

const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

fn get_mixin_key(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    MIXIN_KEY_ENC_TAB
        .iter()
        .filter_map(|&i| chars.get(i).copied())
        .take(32)
        .collect()
}

fn sign_wbi(params: &serde_json::Value, mixin_key: &str) -> String {
    let wts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut all = match params {
        serde_json::Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    all.insert("wts".into(), serde_json::Value::Number(wts.into()));

    let mut keys: Vec<&String> = all.keys().collect();
    keys.sort();

    let query: String = keys
        .iter()
        .map(|k| {
            let v = stringify_wbi_param(&all[*k]);
            let v_cleaned: String = v
                .chars()
                .filter(|c| !matches!(c, '\'' | '!' | '(' | ')' | '*'))
                .collect();
            format!(
                "{}={}",
                urlencoding::encode(k),
                urlencoding::encode(&v_cleaned)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    let mut hasher = Md5::new();
    hasher.update(query.as_bytes());
    hasher.update(mixin_key.as_bytes());
    let w_rid = hex::encode(hasher.finalize());

    format!("{query}&w_rid={w_rid}")
}

fn stringify_wbi_param(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub(super) struct DanmuAuthInfo {
    pub key: String,
    pub urls: Vec<String>,
    pub room_id: u64,
    pub uid: Option<u64>,
    pub buvid3: String,
}

#[derive(serde::Deserialize)]
struct BiliResponse<T> {
    code: i64,
    #[serde(default)]
    message: Option<String>,
    data: Option<T>,
}

#[derive(serde::Deserialize)]
struct SpiData {
    b_3: String,
}

#[derive(serde::Deserialize)]
struct RoomInitData {
    room_id: u64,
}

#[derive(serde::Deserialize)]
struct NavData {
    wbi_img: WbiImg,
    #[serde(default)]
    mid: Option<u64>,
    #[serde(default, rename = "isLogin")]
    is_login: bool,
}

#[derive(serde::Deserialize)]
struct WbiImg {
    img_url: String,
    sub_url: String,
}

#[derive(serde::Deserialize)]
struct DanmuInfoData {
    token: String,
    host_list: Vec<HostEntry>,
}

#[derive(serde::Deserialize)]
struct HostEntry {
    host: String,
    #[serde(default)]
    wss_port: Option<u16>,
}

#[derive(serde::Deserialize)]
struct LoginJson {
    cookie_info: LoginCookieInfo,
}

#[derive(serde::Deserialize)]
struct LoginCookieInfo {
    cookies: Vec<LoginCookie>,
}

#[derive(serde::Deserialize)]
struct LoginCookie {
    name: String,
    value: String,
}

async fn api_get<T: DeserializeOwned>(
    client: &HyperHttpClient,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<T, HttpError> {
    let mut req = hyper::Request::builder()
        .method(hyper::Method::GET)
        .uri(uri(url)?)
        .header("User-Agent", UA);
    for (key, value) in headers {
        req = req.header(*key, *value);
    }
    let body = client.send(req.body(empty_body())?).await?;
    Ok(serde_json::from_slice(&body)?)
}

async fn get_buvid3(client: &HyperHttpClient, cookie: Option<&str>) -> Result<String, HttpError> {
    if let Some(buvid3) = cookie.and_then(|c| cookie_value(c, "buvid3")) {
        return Ok(buvid3);
    }

    let resp: BiliResponse<SpiData> = api_get(
        client,
        "https://api.bilibili.com/x/frontend/finger/spi",
        &[],
    )
    .await?;
    Ok(resp.data.map(|d| d.b_3).unwrap_or_default())
}

async fn resolve_room_id(
    client: &HyperHttpClient,
    room_id: u64,
    cookie: Option<&str>,
) -> Result<u64, HttpError> {
    let cookie_header;
    let headers = if let Some(cookie) = cookie {
        cookie_header = cookie.to_string();
        vec![("Cookie", cookie_header.as_str())]
    } else {
        Vec::new()
    };
    let resp: BiliResponse<RoomInitData> = api_get(
        client,
        &format!("https://api.live.bilibili.com/room/v1/Room/mobileRoomInit?id={room_id}"),
        &headers,
    )
    .await?;
    Ok(resp.data.map(|d| d.room_id).unwrap_or(room_id))
}

pub(super) async fn fetch_danmu_auth_info(
    client: &HyperHttpClient,
    room_id: u64,
    login_json: Option<String>,
) -> Result<DanmuAuthInfo, String> {
    let cookie = login_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(login_json_cookie_header)
        .transpose()?
        .filter(|c| !c.is_empty());
    let buvid3 = get_buvid3(client, cookie.as_deref())
        .await
        .map_err(|e| format!("getBuvid3 failed: {e}"))?;
    let cookie_header = cookie_with_buvid3(cookie.as_deref(), &buvid3);
    let long_room_id = resolve_room_id(client, room_id, Some(&cookie_header))
        .await
        .map_err(|e| format!("resolveRoomId failed: {e}"))?;

    let nav: BiliResponse<NavData> = api_get(
        client,
        "https://api.bilibili.com/x/web-interface/nav",
        &[("Cookie", &cookie_header)],
    )
    .await
    .map_err(|e| format!("nav failed: {e}"))?;

    let nav_data = nav.data.ok_or("nav data missing".to_string())?;
    let uid = nav_data.mid.filter(|uid| *uid > 0);
    if cookie.is_some() && (!nav_data.is_login || uid.is_none()) {
        return Err("Bilibili login JSON is not logged in or has expired".into());
    }

    let img_key = nav_data
        .wbi_img
        .img_url
        .rsplit('/')
        .next()
        .and_then(|s| s.split('.').next())
        .unwrap_or("");
    let sub_key = nav_data
        .wbi_img
        .sub_url
        .rsplit('/')
        .next()
        .and_then(|s| s.split('.').next())
        .unwrap_or("");
    let mixin_key = get_mixin_key(&format!("{img_key}{sub_key}"));

    let signed = sign_wbi(
        &serde_json::json!({
            "id": long_room_id,
            "type": 0,
            "web_location": "444.8"
        }),
        &mixin_key,
    );
    let danmu: BiliResponse<DanmuInfoData> = api_get(
        client,
        &format!("https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?{signed}"),
        &[
            ("Referer", "https://live.bilibili.com/"),
            ("Cookie", &cookie_header),
        ],
    )
    .await
    .map_err(|e| format!("getDanmuInfo failed: {e}"))?;

    if danmu.code != 0 {
        return Err(format!(
            "getDanmuInfo failed: code={} message={}",
            danmu.code,
            danmu.message.unwrap_or_default()
        ));
    }

    let danmu_data = danmu.data.ok_or("danmuInfo data missing")?;
    let urls: Vec<String> = danmu_data
        .host_list
        .iter()
        .map(|h| format!("wss://{}:{}/sub", h.host, h.wss_port.unwrap_or(443)))
        .collect();

    Ok(DanmuAuthInfo {
        key: danmu_data.token,
        urls,
        room_id: long_room_id,
        uid,
        buvid3,
    })
}

pub(super) fn login_json_cookie_header(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let login: LoginJson = serde_json::from_str(trimmed)
        .map_err(|e| format!("parse BiliTV login JSON failed: {e}"))?;
    let parts: Vec<String> = login
        .cookie_info
        .cookies
        .iter()
        .filter_map(|item| {
            let name = item.name.trim();
            let value = item.value.trim();
            if name.is_empty() || value.is_empty() {
                None
            } else {
                Some(format!("{name}={value}"))
            }
        })
        .collect();
    if parts.is_empty() {
        return Err("cookie_info.cookies contains no usable cookies".into());
    }
    Ok(parts.join("; "))
}

fn cookie_value(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key.trim() == name).then(|| value.trim().to_string())
    })
}

fn cookie_with_buvid3(cookie: Option<&str>, buvid3: &str) -> String {
    match cookie {
        Some(cookie) if cookie_value(cookie, "buvid3").is_some() => cookie.to_string(),
        Some(cookie) if !cookie.trim().is_empty() => format!("{}; buvid3={buvid3}", cookie.trim()),
        _ => format!("buvid3={buvid3}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixin_key_length() {
        let key = get_mixin_key(
            "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789",
        );
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_sign_wbi_produces_query() {
        let params = serde_json::json!({"id": 12345});
        let mixin_key = "0123456789abcdef0123456789abcdef";
        let result = sign_wbi(&params, mixin_key);
        assert!(result.contains("wts="));
        assert!(result.contains("w_rid="));
        assert!(result.contains("id=12345"));
    }

    #[test]
    fn test_sign_wbi_string_params_are_not_json_quoted() {
        let params = serde_json::json!({"id": 12345, "web_location": "444.8"});
        let mixin_key = "0123456789abcdef0123456789abcdef";
        let result = sign_wbi(&params, mixin_key);
        assert!(result.contains("web_location=444.8"));
        assert!(!result.contains("web_location=%22444.8%22"));
    }

    #[test]
    fn test_login_json_cookie_header_rejects_cookie_header() {
        assert!(login_json_cookie_header("SESSDATA=abc\nbili_jct=def").is_err());
    }

    #[test]
    fn test_login_json_cookie_header_accepts_login_json() {
        let cookie = login_json_cookie_header(
            r#"{"cookie_info":{"cookies":[{"name":"SESSDATA","value":"abc"},{"name":"DedeUserID","value":"123"}]}}"#,
        )
        .unwrap();
        assert_eq!(cookie, "SESSDATA=abc; DedeUserID=123");
    }
}
