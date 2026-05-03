use md5::{Digest, Md5};

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
            let v = all[*k].to_string();
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

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(serde::Deserialize)]
struct BiliResponse<T> {
    code: i64,
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
}

async fn api_get<T: for<'de> serde::Deserialize<'de>>(
    url: &str,
    headers: Vec<(&str, &str)>,
) -> Result<T, reqwest::Error> {
    let client = reqwest::Client::new();
    let mut req = client.get(url).header("User-Agent", UA);
    for (key, value) in headers {
        req = req.header(key, value);
    }
    req.send().await?.json().await
}

async fn get_buvid3() -> Result<String, reqwest::Error> {
    let resp: BiliResponse<SpiData> =
        api_get("https://api.bilibili.com/x/frontend/finger/spi", vec![]).await?;
    Ok(resp.data.map(|d| d.b_3).unwrap_or_default())
}

async fn resolve_room_id(room_id: u64) -> Result<u64, reqwest::Error> {
    let resp: BiliResponse<RoomInitData> = api_get(
        &format!("https://api.live.bilibili.com/room/v1/Room/mobileRoomInit?id={room_id}"),
        vec![],
    )
    .await?;
    Ok(resp.data.map(|d| d.room_id).unwrap_or(room_id))
}

pub async fn fetch_danmu_info(room_id: u64) -> Result<(String, Vec<String>, u64), String> {
    let buvid3 = get_buvid3()
        .await
        .map_err(|e| format!("getBuvid3 failed: {e}"))?;
    let long_room_id = resolve_room_id(room_id)
        .await
        .map_err(|e| format!("resolveRoomId failed: {e}"))?;

    let nav: BiliResponse<NavData> = api_get(
        "https://api.bilibili.com/x/web-interface/nav",
        vec![("Cookie", &format!("buvid3={buvid3}"))],
    )
    .await
    .map_err(|e| format!("nav failed: {e}"))?;

    let nav_data = nav.data.ok_or("nav data missing".to_string())?;
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

    let signed = sign_wbi(&serde_json::json!({"id": long_room_id}), &mixin_key);
    let danmu: BiliResponse<DanmuInfoData> = api_get(
        &format!("https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?{signed}"),
        vec![
            ("Referer", "https://live.bilibili.com/"),
            ("Cookie", &format!("buvid3={buvid3}")),
        ],
    )
    .await
    .map_err(|e| format!("getDanmuInfo failed: {e}"))?;

    if danmu.code != 0 {
        return Err(format!("getDanmuInfo failed: code={}", danmu.code));
    }

    let danmu_data = danmu.data.ok_or("danmuInfo data missing")?;
    let urls: Vec<String> = danmu_data
        .host_list
        .iter()
        .map(|h| format!("wss://{}/sub", h.host))
        .collect();

    Ok((danmu_data.token, urls, long_room_id))
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
}
