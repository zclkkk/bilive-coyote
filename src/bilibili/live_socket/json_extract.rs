pub fn extract_json_messages(body: &[u8]) -> Vec<serde_json::Value> {
    let text = String::from_utf8_lossy(body);
    let chunks: Vec<&str> = text
        .split(|c: char| c.is_control())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut messages = Vec::new();
    for chunk in chunks {
        let Some(json_start) = chunk.find('{') else {
            continue;
        };
        let json_str = &chunk[json_start..];
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str)
            && parsed.is_object()
        {
            messages.push(parsed);
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_json() {
        let body = br#"{"cmd":"SEND_GIFT","data":{}}"#;
        let msgs = extract_json_messages(body);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["cmd"], "SEND_GIFT");
    }

    #[test]
    fn test_extract_multiple_with_control_chars() {
        let body = b"{\"cmd\":\"A\"}\x00\x01{\"cmd\":\"B\"}";
        let msgs = extract_json_messages(body);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["cmd"], "A");
        assert_eq!(msgs[1]["cmd"], "B");
    }

    #[test]
    fn test_extract_skips_non_json_prefix() {
        let body = b"some garbage {\"cmd\":\"OK\"}";
        let msgs = extract_json_messages(body);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["cmd"], "OK");
    }

    #[test]
    fn test_extract_drops_chunk_with_trailing_garbage() {
        // Matches the TS original: serde_json::from_str rejects trailing text,
        // and we no longer carry a manual brace-scanning fallback.
        let body = b"{\"cmd\":\"OK\"} trailing garbage";
        let msgs = extract_json_messages(body);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_extract_empty() {
        let body = b"";
        let msgs = extract_json_messages(body);
        assert!(msgs.is_empty());
    }
}
