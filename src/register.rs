use anyhow::{anyhow, Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use sp_core::{
    crypto::{AccountId32, Ss58Codec},
    Pair,
};
use std::time::Duration;
use tokio::time::sleep;

use crate::client::*;
use crate::key_utils;
use crate::utils;

// Batch operations configuration
#[derive(Serialize, Deserialize)]
struct BatchConfig {
    operations: Vec<BatchOperation>,
}

#[derive(Serialize, Deserialize)]
struct BatchOperation {
    operation: String, // "register", "check_status", "monitor"
    subnet: u16,
    wallet: Option<String>,
    hotkey: String,
    max_retries: Option<usize>,
}

pub struct QuickRegister {
    client: BittensorClient,
}

impl QuickRegister {
    pub async fn new(endpoint: String) -> Result<Self> {
        let client = BittensorClient::new(endpoint).await?;

        Ok(Self { client })
    }

    pub async fn register_to_subnet(
        &self,
        netuid: u16,
        wallet_path: &str,
        hotkey_path: &str,
        burn_amount: Option<u64>,
    ) -> Result<()> {
        println!(
            "{}",
            "🚀 Starting Bittensor Registration".bright_cyan().bold()
        );
        println!("═══════════════════════════════════════");

        // 1. Loading keys
        let coldkey_pair = key_utils::load_keypair_from_file(wallet_path)
            .context("Failed to load wallet/coldkey")?;
        let hotkey_account =
            key_utils::account_id_from_string(hotkey_path).context("Failed to load hotkey")?;
        let coldkey_account = AccountId32::from(coldkey_pair.public().0);

        println!("🔑 Keys loaded:");
        println!("   Coldkey: {}", coldkey_account.to_ss58check());
        println!("   Hotkey: {}", hotkey_account.to_ss58check());

        // 2. Checking if already registered
        if let Some(neuron) = self
            .client
            .check_registration(netuid, &hotkey_account)
            .await?
        {
            println!(
                "✅ Already registered in subnet {} with UID: {}",
                netuid, neuron.uid
            );
            return Ok(());
        }

        // 3. Getting subnet information
        let subnet_info = self.client.get_subnet_info(netuid, false).await?;

        // 4. Getting the current block number
        let current_block = self.client.get_current_block().await?;
        println!("📦 Current block: {}", current_block);

        // 6. Performing registration using the selected method
        let burn_cost = burn_amount.unwrap_or(subnet_info.burn);
        let registration_data = self
            .perform_burn_registration(
                netuid,
                &hotkey_account,
                &coldkey_account,
                current_block,
                burn_cost,
            )
            .await?;

        // 7. Sending registration
        let tx_hash = self
            .client
            .submit_burned_registration(&registration_data, &coldkey_pair)
            .await?;

        println!("\n🎉 Registration completed successfully!");
        println!("   Transaction hash: {}", tx_hash);
        println!("   Subnet: {}", netuid);
        println!("   Hotkey: {}", hotkey_account.to_ss58check());
        println!("   Coldkey: {}", coldkey_account.to_ss58check());

        // 8. Verifying final registration
        self.verify_registration(netuid, &hotkey_account).await?;

        Ok(())
    }

    // Burn registration
    async fn perform_burn_registration(
        &self,
        netuid: u16,
        hotkey_account: &AccountId32,
        coldkey_account: &AccountId32,
        current_block: u64,
        burn_amount: u64,
    ) -> Result<RegistrationData> {
        println!("\n🔥 Preparing burn registration...");
        println!("   Burn amount: {}", utils::format_tao(burn_amount as u128));

        // Checking balance
        let balance = self.client.get_account_balance(coldkey_account).await?;
        if balance < burn_amount {
            return Err(anyhow!(
                "Insufficient balance. Required: {}, Available: {}",
                utils::format_tao(burn_amount as u128),
                utils::format_tao(balance as u128)
            ));
        }

        println!("✅ Sufficient balance confirmed");

        Ok(RegistrationData {
            subnet_id: netuid,
            hotkey: hotkey_account.clone(),
            coldkey: coldkey_account.clone(),
            burn_amount: burn_amount,
            block_number: current_block,
        })
    }

    // Verification of registration success
    async fn verify_registration(&self, netuid: u16, hotkey_account: &AccountId32) -> Result<()> {
        println!("\n🔍 Verifying registration...");

        for attempt in 1..=5 {
            println!("   Attempt {}/5...", attempt);
            sleep(Duration::from_secs(12)).await;

            match self
                .client
                .check_registration(netuid, hotkey_account)
                .await?
            {
                Some(neuron) => {
                    println!("✅ Registration verified! Assigned UID: {}", neuron.uid);
                    println!("   Active: {}", if neuron.active { "Yes" } else { "No" });
                    println!(
                        "   Stake: {}",
                        utils::format_tao(neuron.stake.iter().map(|(_, s)| s).sum::<u64>() as u128)
                    );
                    return Ok(());
                }
                None => {
                    if attempt < 5 {
                        println!("   Still processing, waiting for next block...");
                    }
                }
            }
        }

        println!(
            "⚠️ Registration may still be processing. Check status manually in a few minutes."
        );
        Ok(())
    }

    pub async fn estimate_registration_cost(&self, netuid: u16) -> Result<()> {
        println!("💰 Estimating registration costs for subnet {}...", netuid);
        println!("═══════════════════════════════════════════════════");

        let subnet_info = self.client.get_subnet_info(netuid, false).await?;

        println!("\n📊 Cost Analysis:");
        println!("┌─ Burn Registration (Instant)");
        println!(
            "│  ├─ Cost: {}",
            utils::format_tao(subnet_info.burn as u128)
        );
        println!(
            "│  ├─ USD equivalent: ~${:.2} (assuming $200/TAO)",
            subnet_info.burn as f64 / 1e9 * 200.0
        );
        println!("│  └─ Processing time: 1-2 blocks (~12-24s)");

        Ok(())
    }

    pub async fn check_status(&self, netuid: u16, hotkey_path: &str) -> Result<()> {
        println!("🔍 Checking registration status...");

        let hotkey_account =
            key_utils::account_id_from_string(hotkey_path).context("Failed to load hotkey")?;

        match self
            .client
            .check_registration(netuid, &hotkey_account)
            .await?
        {
            Some(neuron) => {
                println!("✅ Neuron is registered in subnet {}!", netuid);
                println!("\n📊 Neuron Details:");
                println!("   UID: {}", neuron.uid);
                println!("   Hotkey: {}", neuron.hotkey.to_ss58check());
                println!("   Coldkey: {}", neuron.coldkey.to_ss58check());
                println!("   Active: {}", if neuron.active { "Yes" } else { "No" });
                println!(
                    "   Stake: {}",
                    utils::format_tao(neuron.stake.iter().map(|(_, s)| s).sum::<u64>() as u128)
                );
                println!("   Emission: {}", neuron.emission);
                println!("   Last update: block {}", neuron.last_update);
                println!("   Validator permit: {}", neuron.validator_permit);

                // Show additional statistics
                let subnet_info = self.client.get_subnet_info(netuid, false).await?;
                println!("\n📈 Subnet Statistics:");
                println!(
                    "   Total neurons: {}/{}",
                    subnet_info.registered_neurons, subnet_info.max_allowed_uids
                );
                println!("   Registration difficulty: {}", subnet_info.difficulty);
                println!(
                    "   Burn cost: {}",
                    utils::format_tao(subnet_info.burn as u128)
                );
            }
            None => {
                println!(
                    "❌ Hotkey {} is NOT registered in subnet {}",
                    hotkey_account.to_ss58check(),
                    netuid
                );

                // Show possible registration information
                let subnet_info = self.client.get_subnet_info(netuid, false).await?;
                println!("\n💡 Registration options:");
                println!(
                    "   Burn cost: {}",
                    utils::format_tao(subnet_info.burn as u128)
                );
            }
        }

        Ok(())
    }

    pub async fn show_subnet_info(&self, netuid: u16) -> Result<()> {
        println!("📋 Fetching subnet {} information...", netuid);

        let subnet_info = self.client.get_subnet_info(netuid, true).await?;

        println!("\n📊 Subnet {} Details:", netuid);
        println!("═══════════════════════════════════════");
        println!(
            "   Registered neurons: {}/{}",
            subnet_info.subnetwork_n, subnet_info.max_n
        );
        println!("   Registration difficulty: {}", subnet_info.difficulty);
        println!(
            "   Burn cost: {}",
            utils::format_tao(subnet_info.burn as u128)
        );
        println!("   Tempo: {} blocks", subnet_info.tempo);
        println!("   Immunity period: {} blocks", subnet_info.immunity_period);
        println!(
            "   Min allowed weights: {}",
            subnet_info.min_allowed_weights
        );
        println!("   Max weight limit: {}", subnet_info.max_weight_limit);
        println!(
            "   Max allowed validators: {}",
            subnet_info.max_allowed_validators
        );
        println!(
            "   Owner: {}",
            utils::format_ss58_short(&subnet_info.owner_ss58)
        );
        println!("   Network modality: {}", subnet_info.modality);
        println!("   Emission value: {}", subnet_info.emission_value);
        println!("   Rho: {}", subnet_info.rho);
        println!("   Kappa: {}", subnet_info.kappa);
        println!("   Scaling law power: {}", subnet_info.scaling_law_power);
        println!("   Blocks since epoch: {}", subnet_info.blocks_since_epoch);

        // Show registration statistics
        let current_block = self.client.get_current_block().await?;

        println!("\n⏱️ Registration Estimates:");
        println!("   Current block: {}", current_block);
        println!(
            "   Burn cost in USD: ~${:.2}",
            subnet_info.burn as f64 / 1e9 * 200.0
        );

        Ok(())
    }

    // Massive monitoring of multiple neurons
    pub async fn monitor_multiple_neurons(&self, registrations: Vec<(u16, String)>) -> Result<()> {
        println!("👀 Monitoring {} registration(s)...", registrations.len());
        println!("═══════════════════════════════════════════");

        for (netuid, hotkey_path) in registrations {
            println!(
                "\n📍 Subnet {} - {}",
                netuid,
                utils::format_account_short(&key_utils::account_id_from_string(&hotkey_path)?)
            );

            match self.check_status(netuid, &hotkey_path).await {
                Ok(_) => {}
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        Ok(())
    }

    // Automatic registration with retry logic
    pub async fn auto_register_with_retry(
        &self,
        netuid: u16,
        wallet_path: &str,
        hotkey_path: &str,
        max_retries: usize,
    ) -> Result<()> {
        println!(
            "🔄 Auto registration with retry (max {} attempts)",
            max_retries
        );

        for attempt in 1..=max_retries {
            println!("\n🚀 Registration attempt {}/{}", attempt, max_retries);

            match self
                .register_to_subnet(netuid, wallet_path, hotkey_path, None)
                .await
            {
                Ok(_) => {
                    println!("✅ Registration successful on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    println!("❌ Attempt {} failed: {}", attempt, e);
                    if attempt < max_retries {
                        println!("⏳ Waiting 30s before retry...");
                        sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        }

        Err(anyhow!("All registration attempts failed"))
    }

    /// This function provides an overview of the Bittensor network, including active subnets,
    pub async fn show_network_statistics(&self) -> Result<()> {
        println!("📊 Bittensor Network Statistics");
        println!("═══════════════════════════════════════");

        // Getting information for basic subnets
        let main_subnets = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let mut total_neurons = 0u32;
        let mut active_subnets = 0u32;

        println!("\n🌐 Active Subnets:");
        println!("┌─────┬─────────────┬──────────┬─────────────┬──────────────┐");
        println!("│ UID │   Neurons   │ Max Cap  │  Burn Cost  │ Difficulty   │");
        println!("├─────┼─────────────┼──────────┼─────────────┼──────────────┤");

        for netuid in main_subnets {
            match self.client.get_subnet_info(netuid, false).await {
                Ok(subnet_info) => {
                    active_subnets += 1;
                    total_neurons += subnet_info.registered_neurons as u32;

                    println!(
                        "│ {:>3} │ {:>7}/{:<3} │ {:>8} │ {:>9} │ {:>10} │",
                        netuid,
                        subnet_info.registered_neurons,
                        subnet_info.max_allowed_uids,
                        subnet_info.max_allowed_uids,
                        utils::format_tao(subnet_info.burn as u128),
                        utils::format_difficulty(subnet_info.difficulty),
                    );
                }
                Err(_) => {
                    // Subnet doesn't exist or error, skip
                }
            }
        }

        println!("└─────┴─────────────┴──────────┴─────────────┴──────────────┘");

        // Common statistics
        let current_block = self.client.get_current_block().await?;

        println!("\n📈 Network Overview:");
        println!("   Active subnets: {}", active_subnets);
        println!("   Total neurons: {:?}", total_neurons);
        println!("   Current block: {:?}", current_block);
        println!("   Network: Finney (Mainnet)");

        // New users recomendations
        println!("\n💡 Registration Tips:");
        println!("   • Subnet 1: Text generation (high competition)");
        println!("   • Subnet 3: Data scraping (moderate difficulty)");
        println!("   • Subnet 8: Time series prediction");
        println!("   • Check difficulty before registering");
        println!("   • Consider burn registration for high-difficulty subnets");

        Ok(())
    }

    // Export configuration for automation
    pub async fn export_config(&self, netuid: u16, output_path: &str) -> Result<()> {
        println!("📄 Exporting configuration for subnet {}...", netuid);

        let subnet_info = self.client.get_subnet_info(netuid, true).await?;

        let config = serde_json::json!({
            "subnet_id": netuid,
            "registration_info": {
                "difficulty": subnet_info.difficulty.to_string(),
                "burn_cost_rao": subnet_info.burn,
                "burn_cost_tao": subnet_info.burn as f64 / 1e9,
                "max_neurons": subnet_info.max_allowed_uids,
                "current_neurons": subnet_info.registered_neurons,
                "registration_open": subnet_info.registered_neurons < subnet_info.max_allowed_uids
            },
            "export_time": chrono::Utc::now().to_rfc3339(),
            "network": "finney"
        });

        std::fs::write(output_path, serde_json::to_string_pretty(&config)?)?;
        println!("✅ Configuration exported to: {}", output_path);

        Ok(())
    }

    pub async fn execute_batch_operations(&self, config_path: &str) -> Result<()> {
        println!("📦 Executing batch operations from: {}", config_path);

        let config_content = std::fs::read_to_string(config_path)?;
        let batch_config: BatchConfig = serde_json::from_str(&config_content)?;

        println!("   Found {} operations", batch_config.operations.len());

        for (i, operation) in batch_config.operations.iter().enumerate() {
            println!(
                "\n🔄 Operation {}/{}: {}",
                i + 1,
                batch_config.operations.len(),
                operation.operation
            );

            match operation.operation.as_str() {
                "register" => {
                    if let Some(wallet) = &operation.wallet {
                        match self
                            .register_to_subnet(operation.subnet, wallet, &operation.hotkey, None)
                            .await
                        {
                            Ok(_) => println!("✅ Registration completed"),
                            Err(e) => println!("❌ Registration failed: {}", e),
                        }
                    }
                }
                "check_status" => {
                    match self.check_status(operation.subnet, &operation.hotkey).await {
                        Ok(_) => {}
                        Err(e) => println!("❌ Status check failed: {}", e),
                    }
                }
                "auto_register" => {
                    if let Some(wallet) = &operation.wallet {
                        let max_retries = operation.max_retries.unwrap_or(3);
                        match self
                            .auto_register_with_retry(
                                operation.subnet,
                                wallet,
                                &operation.hotkey,
                                max_retries,
                            )
                            .await
                        {
                            Ok(_) => println!("✅ Auto registration completed"),
                            Err(e) => println!("❌ Auto registration failed: {}", e),
                        }
                    }
                }
                _ => {
                    println!("⚠️ Unknown operation: {}", operation.operation);
                }
            }

            // Small delay between operations
            if i < batch_config.operations.len() - 1 {
                println!("⏳ Waiting 5s before next operation...");
                sleep(Duration::from_secs(5)).await;
            }
        }

        println!("\n🎉 Batch operations completed!");
        Ok(())
    }

    // Check account balance
    pub async fn check_account_balance(&self, account_address: &str) -> Result<()> {
        println!("💰 Checking account balance...");

        // Parse the account address using SS58 codec
        let account = AccountId32::from_ss58check(account_address).map_err(|e| {
            anyhow::anyhow!(
                "Invalid SS58 account address format: {:?}. Address: {}",
                e,
                account_address
            )
        })?;

        // Get account info with debug output
        match self.client.get_account_balance(&account).await {
            Ok(balance) => {
                println!("✅ Account balance retrieved successfully!");
                println!("💰 Address: {}", account_address);
                println!("💰 Balance: {} RAO", balance);
                println!("💰 Balance: {:.6} TAO", utils::format_tao(balance as u128));

                if balance == 0 {
                    println!("ℹ️ Note: Account has zero balance or doesn't exist on-chain");
                }
            }
            Err(e) => {
                println!("❌ Failed to get account balance: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let result = QuickRegister::new("wss://test.example.com".to_string()).await;
        // Will not collected in test environment but structure should creates
        assert!(result.is_err());
    }
}
