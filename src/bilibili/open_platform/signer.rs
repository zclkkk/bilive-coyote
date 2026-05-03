use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn sign_open_platform_request(
    params: &serde_json::Value,
    app_key: &str,
    app_secret: &str,
) -> std::collections::HashMap<String, String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let nonce = timestamp + (rand::random::<u64>() % 100000);

    let params_str = serde_json::to_string(params).unwrap_or_default();
    let mut hasher = Md5::new();
    hasher.update(params_str.as_bytes());
    let md5_hex = hex::encode(hasher.finalize());

    let mut headers = std::collections::HashMap::new();
    headers.insert("x-bili-accesskeyid".into(), app_key.into());
    headers.insert("x-bili-content-md5".into(), md5_hex);
    headers.insert("x-bili-signature-method".into(), "HMAC-SHA256".into());
    headers.insert("x-bili-signature-nonce".into(), nonce.to_string());
    headers.insert("x-bili-signature-version".into(), "1.0".into());
    headers.insert("x-bili-timestamp".into(), timestamp.to_string());

    let ordered_keys = [
        "x-bili-accesskeyid",
        "x-bili-content-md5",
        "x-bili-signature-method",
        "x-bili-signature-nonce",
        "x-bili-signature-version",
        "x-bili-timestamp",
    ];

    let data = ordered_keys
        .iter()
        .map(|key| format!("{key}:{}", headers.get(*key).unwrap()))
        .collect::<Vec<_>>()
        .join("\n");

    let mut mac = HmacSha256::new_from_slice(app_secret.as_bytes()).unwrap();
    mac.update(data.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    headers.insert("Content-Type".into(), "application/json".into());
    headers.insert("Accept".into(), "application/json".into());
    headers.insert("Authorization".into(), signature);

    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_produces_required_headers() {
        let params = serde_json::json!({"code": "test", "app_id": 1});
        let headers = sign_open_platform_request(&params, "mykey", "mysecret");

        assert_eq!(headers.get("x-bili-accesskeyid").unwrap(), "mykey");
        assert!(headers.contains_key("x-bili-content-md5"));
        assert_eq!(
            headers.get("x-bili-signature-method").unwrap(),
            "HMAC-SHA256"
        );
        assert!(headers.contains_key("x-bili-signature-nonce"));
        assert_eq!(headers.get("x-bili-signature-version").unwrap(), "1.0");
        assert!(headers.contains_key("x-bili-timestamp"));
        assert!(headers.contains_key("Authorization"));
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");
    }

    #[test]
    fn test_sign_deterministic_for_same_inputs() {
        let params = serde_json::json!({"code": "test"});
        let h1 = sign_open_platform_request(&params, "key", "secret");
        let h2 = sign_open_platform_request(&params, "key", "secret");
        assert_eq!(h1.get("x-bili-content-md5"), h2.get("x-bili-content-md5"));
    }

    #[test]
    fn test_sign_authorization_is_valid_hmac() {
        let params = serde_json::json!({"code": "test", "app_id": 1});
        let headers = sign_open_platform_request(&params, "mykey", "mysecret");

        let auth = headers.get("Authorization").expect("Authorization header missing");
        assert!(!auth.is_empty(), "Authorization must not be empty");

        let ordered_keys = [
            "x-bili-accesskeyid",
            "x-bili-content-md5",
            "x-bili-signature-method",
            "x-bili-signature-nonce",
            "x-bili-signature-version",
            "x-bili-timestamp",
        ];

        let reconstructed = ordered_keys
            .iter()
            .map(|key| format!("{key}:{}", headers.get(*key).unwrap()))
            .collect::<Vec<_>>()
            .join("\n");

        let mut mac = HmacSha256::new_from_slice(b"mysecret").unwrap();
        mac.update(reconstructed.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());
        assert_eq!(auth, &expected, "Authorization must match reconstructed HMAC");
    }
}
