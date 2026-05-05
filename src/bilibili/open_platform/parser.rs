use crate::config::types::GiftEvent;

const CMD_OPEN_PLATFORM_GIFT: &str = "LIVE_OPEN_PLATFORM_SEND_GIFT";

#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
struct OpenPlatformGiftData {
    gift_id: Option<u64>,
    giftId: Option<u64>,
    gift_name: Option<String>,
    giftName: Option<String>,
    paid: Option<bool>,
    price: Option<u64>,
    total_coin: Option<u64>,
    gift_num: Option<u32>,
    num: Option<u32>,
    uid: Option<u64>,
    uname: Option<String>,
    username: Option<String>,
    timestamp: Option<u64>,
}

#[derive(serde::Deserialize)]
struct OpenPlatformGiftMessage {
    cmd: String,
    data: Option<OpenPlatformGiftData>,
}

pub fn parse_open_platform_gift(message: &serde_json::Value) -> Option<GiftEvent> {
    let msg: OpenPlatformGiftMessage = serde_json::from_value(message.clone()).ok()?;

    if msg.cmd != CMD_OPEN_PLATFORM_GIFT {
        return None;
    }

    let data = msg.data?;
    let gift_name = data.gift_name.or(data.giftName).filter(|s| !s.is_empty())?;
    let uname = data.uname.or(data.username).filter(|s| !s.is_empty())?;
    let paid = data.paid?;

    Some(GiftEvent {
        gift_id: data.gift_id.or(data.giftId)?,
        gift_name,
        coin_type: if paid { "gold".into() } else { "silver".into() },
        total_coin: data.price.or(data.total_coin)?,
        num: data.gift_num.or(data.num)?,
        uid: data.uid?,
        uname,
        timestamp: data.timestamp?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gift() {
        let msg = serde_json::json!({
            "cmd": "LIVE_OPEN_PLATFORM_SEND_GIFT",
            "data": {
                "gift_id": 123,
                "gift_name": "test",
                "paid": true,
                "price": 100,
                "num": 2,
                "uid": 456,
                "uname": "user",
                "timestamp": 1700000000
            }
        });
        let gift = parse_open_platform_gift(&msg).unwrap();
        assert_eq!(gift.gift_id, 123);
        assert_eq!(gift.gift_name, "test");
        assert_eq!(gift.coin_type, "gold");
        assert_eq!(gift.total_coin, 100);
        assert_eq!(gift.num, 2);
        assert_eq!(gift.uid, 456);
        assert_eq!(gift.uname, "user");
    }

    #[test]
    fn test_parse_gift_wrong_cmd() {
        let msg = serde_json::json!({"cmd": "OTHER", "data": {}});
        assert!(parse_open_platform_gift(&msg).is_none());
    }

    #[test]
    fn test_parse_gift_no_data() {
        let msg = serde_json::json!({"cmd": "LIVE_OPEN_PLATFORM_SEND_GIFT"});
        assert!(parse_open_platform_gift(&msg).is_none());
    }

    #[test]
    fn test_parse_gift_alt_fields() {
        let msg = serde_json::json!({
            "cmd": "LIVE_OPEN_PLATFORM_SEND_GIFT",
            "data": {
                "giftId": 999,
                "giftName": "alt",
                "paid": false,
                "total_coin": 50,
                "gift_num": 3,
                "uid": 456,
                "username": "altuser",
                "timestamp": 1700000000
            }
        });
        let gift = parse_open_platform_gift(&msg).unwrap();
        assert_eq!(gift.gift_id, 999);
        assert_eq!(gift.gift_name, "alt");
        assert_eq!(gift.coin_type, "silver");
        assert_eq!(gift.total_coin, 50);
        assert_eq!(gift.num, 3);
        assert_eq!(gift.uname, "altuser");
    }

    #[test]
    fn test_parse_gift_rejects_partial_data() {
        let msg = serde_json::json!({
            "cmd": "LIVE_OPEN_PLATFORM_SEND_GIFT",
            "data": {
                "giftId": 999,
                "giftName": "alt"
            }
        });
        assert!(parse_open_platform_gift(&msg).is_none());
    }
}
