//! Utility functions for Bittensor registration
use sp_core::{
    crypto::{AccountId32, Ss58Codec},
    U256,
};
use std::time::Duration;
use crate::constants::RAO_PER_TAO;

pub fn format_tao(rao: u64) -> String {
    let tao = rao as f64 / RAO_PER_TAO as f64;
    if tao >= 1000.0 {
        format!("{:.1}K TAO", tao / 1000.0)
    } else if tao >= 1.0 {
        format!("{:.3} TAO", tao)
    } else if rao >= 1_000_000 {
        format!("{:.1}M RAO", rao as f64 / 1e6)
    } else if rao >= 1_000 {
        format!("{:.1}K RAO", rao as f64 / 1e3)
    } else {
        format!("{} RAO", rao)
    }
}

pub fn format_hash_rate(attempts: u64, duration: Duration) -> String {
    let rate = attempts as f64 / duration.as_secs_f64();
    if rate >= 1_000_000.0 {
        format!("{:.2} MH/s", rate / 1_000_000.0)
    } else if rate >= 1_000.0 {
        format!("{:.2} KH/s", rate / 1_000.0)
    } else {
        format!("{:.2} H/s", rate)
    }
}

pub fn format_account_short(account: &AccountId32) -> String {
    let ss58 = account.to_ss58check();
    format_ss58_short(&ss58)
}

pub fn format_ss58_short(ss58: &str) -> String {
    if ss58.len() > 16 {
        format!("{}...{}", &ss58[..8], &ss58[ss58.len()-8..])
    } else {
        ss58.to_string()
    }
}

pub fn format_difficulty(difficulty: U256) -> String {
    if difficulty > U256::from(1_000_000_000_000_000_000u64) {
        format!("{:.2}E", difficulty.as_u128() as f64 / 1e18)
    } else if difficulty > U256::from(1_000_000_000_000_000u64) {
        format!("{:.2}P", difficulty.as_u128() as f64 / 1e15)
    } else if difficulty > U256::from(1_000_000_000_000u64) {
        format!("{:.2}T", difficulty.as_u128() as f64 / 1e12)
    } else if difficulty > U256::from(1_000_000_000u64) {
        format!("{:.2}G", difficulty.as_u128() as f64 / 1e9)
    } else if difficulty > U256::from(1_000_000u64) {
        format!("{:.2}M", difficulty.as_u128() as f64 / 1e6)
    } else {
        difficulty.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_utils() {
        assert_eq!(format_tao(1_000_000_000), "1.000 TAO");
        assert_eq!(format_tao(500_000_000), "500.0M RAO");
        assert_eq!(format_tao(1000), "1.0K RAO");
        
        let hash_rate = format_hash_rate(50000, Duration::from_secs(10));
        assert!(hash_rate.contains("KH/s"));
    }
}