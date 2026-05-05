use crate::config::types::{Channel, CoinType, GiftEvent, GiftRule, RuleChannel};
use crate::engine::types::{GiftLogEvent, StrengthChangeEvent, StrengthSource};

pub fn match_rule(rule: &GiftRule, gift: &GiftEvent) -> bool {
    if let Some(rule_gift_id) = rule.gift_id
        && gift.gift_id != rule_gift_id
    {
        return false;
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
    let channels = rule_channels(rule.channel);
    let raw_delta = (rule.strength_add as u64).saturating_mul(gift.num as u64);
    let delta_i32 = i32::try_from(raw_delta).unwrap_or(i32::MAX);

    let events: Vec<StrengthChangeEvent> = channels
        .iter()
        .filter(|_| raw_delta > 0)
        .map(|&ch| StrengthChangeEvent {
            channel: ch,
            delta: delta_i32,
            source: StrengthSource::Gift,
            gift_name: Some(gift.gift_name.clone()),
            uname: Some(gift.uname.clone()),
            duration: Some(rule.duration),
        })
        .collect();

    let mut effects = Vec::new();
    if raw_delta > 0 {
        effects.extend(channels.iter().map(|ch| format!("{ch:?}+{raw_delta}")));
    }
    if let Some(waveform) = rule.waveform.as_ref() {
        effects.push(format!("波形:{waveform}"));
    }

    (events, effects.join(" "))
}

pub fn rule_channels(channel: RuleChannel) -> Vec<Channel> {
    match channel {
        RuleChannel::Both => vec![Channel::A, Channel::B],
        RuleChannel::A => vec![Channel::A],
        RuleChannel::B => vec![Channel::B],
    }
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
