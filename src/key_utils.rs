//! Key Utility functions for Bittensor registration
use anyhow::{Context, Result, *};
use serde::Deserialize;
use sp_core::{
    crypto::{AccountId32, Ss58Codec},
    sr25519::Pair as Sr25519Pair,
    Pair,
};

use std::fs;

pub fn load_keypair_from_file(path: &str) -> Result<Sr25519Pair> {
    if path.starts_with("//") {
        // Dev key (//Alice, //Bob, etc.)
        println!("🔑 Using dev key: {}", path);
        Ok(Sr25519Pair::from_string(path, None)?)
    } else if std::path::Path::new(path).exists() {
        // File path
        let contents =
            fs::read_to_string(path).context(format!("Failed to read key file: {}", path))?;

        // Try different formats
        if contents.trim().starts_with('{') {
            // JSON format
            #[derive(Deserialize)]
            struct KeyFile {
                #[serde(alias = "secretSeed", alias = "seed")]
                secret_seed: Option<String>,
                #[serde(alias = "secretPhrase", alias = "phrase")]
                secret_phrase: Option<String>,
            }

            let key_data: KeyFile =
                serde_json::from_str(&contents).context("Invalid JSON key file format")?;

            if let Some(seed) = key_data.secret_seed {
                Ok(Sr25519Pair::from_string(&seed, None)?)
            } else if let Some(phrase) = key_data.secret_phrase {
                Ok(Sr25519Pair::from_string(&phrase, None)?)
            } else {
                Err(anyhow!("Key file missing secretSeed or secretPhrase"))
            }
        } else {
            // Raw seed/phrase format
            let seed = contents.trim();
            Ok(Sr25519Pair::from_string(seed, None)?)
        }
    } else {
        // Direct seed/phrase
        println!("🔑 Using provided seed/phrase");
        Ok(Sr25519Pair::from_string(path, None)?)
    }
}

pub fn account_id_from_string(account: &str) -> Result<AccountId32> {
    if account.starts_with("//") {
        // Dev key
        let pair = Sr25519Pair::from_string(account, None)?;
        Ok(AccountId32::from(pair.public().0))
    } else if std::path::Path::new(account).exists() {
        // File path - load public key from file
        let pair = load_keypair_from_file(account)?;
        Ok(AccountId32::from(pair.public().0))
    } else if account.len() == 48 && account.chars().all(|c| c.is_ascii_alphanumeric()) {
        // SS58 address
        AccountId32::from_ss58check(account).map_err(|_| anyhow!("Invalid SS58 address"))
    } else if account.len() != 0 {
        // Try as raw seed/phrase to get public
        let pair = Sr25519Pair::from_string(account, None)?;
        Ok(AccountId32::from(pair.public().0))
    } else {
        Err(anyhow!("Empty account string provided"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_parsing() {
        // Test dev key
        let result = account_id_from_string("//Alice");
        assert!(result.is_ok());

        // Test invalid input
        let result = account_id_from_string("");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_key_loading() {
        // Load keys from seed phrase
        let seed = "bottom drive obey lake curtain smoke basket hold race lonely fit walk";
        let result = load_keypair_from_file(seed);
        assert!(result.is_ok());
    }
}
