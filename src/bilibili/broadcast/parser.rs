use crate::config::types::{GiftCoinType, GiftEvent};

const CMD_BROADCAST_GIFT: &str = "SEND_GIFT";

#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
struct BroadcastGiftData {
    #[serde(default)]
    giftId: Option<u64>,
    #[serde(default)]
    giftName: Option<String>,
    #[serde(default)]
    coin_type: Option<String>,
    #[serde(default)]
    price: Option<u64>,
    #[serde(default)]
    num: Option<u32>,
    #[serde(default)]
    uid: Option<u64>,
    #[serde(default)]
    uname: Option<String>,
    #[serde(default)]
    timestamp: Option<u64>,
}

#[derive(serde::Deserialize)]
struct BroadcastGiftMessage {
    cmd: String,
    data: Option<BroadcastGiftData>,
}

pub fn parse_broadcast_gift(message: &serde_json::Value) -> Option<GiftEvent> {
    let msg: BroadcastGiftMessage = serde_json::from_value(message.clone()).ok()?;

    if msg.cmd != CMD_BROADCAST_GIFT {
        return None;
    }

    let d = msg.data?;
    let gift_name = d.giftName.filter(|s| !s.is_empty())?;
    let coin_type = GiftCoinType::parse(d.coin_type.as_deref()?)?;
    let uname = d.uname.filter(|s| !s.is_empty())?;

    Some(GiftEvent {
        gift_id: d.giftId?,
        gift_name,
        coin_type,
        total_coin: d.price?,
        num: d.num?,
        uid: d.uid?,
        uname,
        timestamp: d.timestamp?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gift() {
        let msg = serde_json::json!({
            "cmd": "SEND_GIFT",
            "data": {
                "giftId": 123,
                "giftName": "test",
                "coin_type": "gold",
                "price": 100,
                "num": 2,
                "uid": 456,
                "uname": "user",
                "timestamp": 1700000000
            }
        });
        let gift = parse_broadcast_gift(&msg).unwrap();
        assert_eq!(gift.gift_id, 123);
        assert_eq!(gift.gift_name, "test");
        assert_eq!(gift.coin_type, GiftCoinType::Gold);
        assert_eq!(gift.total_coin, 100);
        assert_eq!(gift.num, 2);
    }

    #[test]
    fn test_parse_gift_wrong_cmd() {
        let msg = serde_json::json!({"cmd": "OTHER", "data": {}});
        assert!(parse_broadcast_gift(&msg).is_none());
    }

    #[test]
    fn test_parse_gift_no_data() {
        let msg = serde_json::json!({"cmd": "SEND_GIFT"});
        assert!(parse_broadcast_gift(&msg).is_none());
    }

    #[test]
    fn test_parse_gift_rejects_partial_data() {
        let msg = serde_json::json!({
            "cmd": "SEND_GIFT",
            "data": {
                "giftId": 123,
                "giftName": "test"
            }
        });
        assert!(parse_broadcast_gift(&msg).is_none());
    }

    #[test]
    fn test_parse_gift_rejects_unknown_coin_type() {
        let msg = serde_json::json!({
            "cmd": "SEND_GIFT",
            "data": {
                "giftId": 123,
                "giftName": "test",
                "coin_type": "unknown",
                "price": 100,
                "num": 2,
                "uid": 456,
                "uname": "user",
                "timestamp": 1700000000
            }
        });
        assert!(parse_broadcast_gift(&msg).is_none());
    }
}
