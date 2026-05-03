pub fn extract_json_messages(body: &[u8]) -> Vec<serde_json::Value> {
    let text = String::from_utf8_lossy(body);
    let chunks: Vec<&str> = text
        .split(|c: char| c.is_control())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let source = if chunks.is_empty() {
        vec![text.as_ref()]
    } else {
        chunks
    };

    let mut messages = Vec::new();
    for chunk in source {
        if let Some(json_start) = chunk.find('{') {
            let json_str = &chunk[json_start..];
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                if parsed.is_object() {
                    messages.push(parsed);
                }
            } else {
                let end = find_json_end(json_str);
                if end > 0 {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str[..end])
                    {
                        if parsed.is_object() {
                            messages.push(parsed);
                        }
                    }
                }
            }
        }
    }
    messages
}

fn find_json_end(s: &str) -> usize {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if escape {
            escape = false;
        } else if in_string {
            if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    0
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
    fn test_extract_skips_non_json() {
        let body = b"some garbage {\"cmd\":\"OK\"} more garbage";
        let msgs = extract_json_messages(body);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["cmd"], "OK");
    }

    #[test]
    fn test_extract_empty() {
        let body = b"";
        let msgs = extract_json_messages(body);
        assert!(msgs.is_empty());
    }
}
