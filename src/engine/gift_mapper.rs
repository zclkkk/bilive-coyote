use crate::config::types::{Channel, CoinType, GiftEvent, GiftRule, RuleChannel};
use crate::engine::types::{GiftLogEvent, StrengthChangeEvent, StrengthSource};

pub fn match_rule(rule: &GiftRule, gift: &GiftEvent) -> bool {
    if let Some(rule_gift_id) = rule.gift_id {
        if gift.gift_id != rule_gift_id {
            return false;
        }
    }
    if gift.gift_name != rule.gift_name {
        return false;
    }
    if rule.coin_type != CoinType::All && rule.coin_type.as_str() != gift.coin_type {
        return false;
    }
    true
}

pub fn apply_rule(rule: &GiftRule, gift: &GiftEvent) -> (Vec<StrengthChangeEvent>, String) {
    let channels: Vec<Channel> = match rule.channel {
        RuleChannel::Both => vec![Channel::A, Channel::B],
        RuleChannel::A => vec![Channel::A],
        RuleChannel::B => vec![Channel::B],
    };
    let raw_delta = (rule.strength_add as u64).saturating_mul(gift.num as u64);
    let delta_i32 = i32::try_from(raw_delta).unwrap_or(i32::MAX);

    let events: Vec<StrengthChangeEvent> = channels
        .iter()
        .map(|&ch| StrengthChangeEvent {
            channel: ch,
            delta: delta_i32,
            absolute: None,
            source: StrengthSource::Gift,
            gift_name: Some(gift.gift_name.clone()),
            uname: Some(gift.uname.clone()),
            duration: Some(rule.duration),
        })
        .collect();

    let delta_str = channels
        .iter()
        .map(|ch| format!("{ch:?}+{raw_delta}"))
        .collect::<Vec<_>>()
        .join(" ");

    (events, delta_str)
}

impl CoinType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CoinType::Gold => "gold",
            CoinType::Silver => "silver",
            CoinType::All => "all",
        }
    }
}

pub fn build_gift_log(gift: &GiftEvent, strength_delta: String) -> GiftLogEvent {
    GiftLogEvent {
        gift_id: gift.gift_id,
        gift_name: gift.gift_name.clone(),
        coin_type: gift.coin_type.clone(),
        total_coin: gift.total_coin,
        num: gift.num,
        uid: gift.uid,
        uname: gift.uname.clone(),
        timestamp: gift.timestamp,
        strength_delta,
    }
}
