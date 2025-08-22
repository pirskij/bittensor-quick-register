use anyhow::{anyhow, Context, Result};
use codec::{Decode, Encode};
use jsonrpsee::{core::client::ClientT, rpc_params, ws_client::WsClientBuilder};
use primitive_types::{H256, U256};
use serde::{Deserialize, Serialize};
use sp_core::{
    blake2_128,
    crypto::{AccountId32, Ss58Codec},
    sr25519::Pair as Sr25519Pair,
    twox_128, Pair,
};
use std::{str::FromStr, time::Duration};

use crate::utils;

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationData {
    pub subnet_id: u16,
    pub hotkey: AccountId32,
    pub coldkey: AccountId32,
    pub burn_amount: u64, // In RAO
    pub block_number: u64,
}

#[derive(Debug, Clone)]
pub struct SubnetInfo {
    pub netuid: u16,
    pub difficulty: U256,
    pub immunity_period: u16,
    pub min_allowed_weights: u16,
    pub max_weight_limit: u16,
    pub max_allowed_validators: u16,
    pub max_n: u16, // max neurons
    pub tempo: u16,
    pub burn: u64,          // Burned registration cost in RAO
    pub owner_ss58: String, // Owner as SS58 address
    pub emission_value: u64,
    pub rho: u16,
    pub kappa: u16,
    pub scaling_law_power: u16,
    pub subnetwork_n: u16, // Current number of registered neurons
    pub blocks_since_epoch: u64,
    pub modality: u16,
    // Legacy fields for backward compatibility
    pub network_modality: u16, // Same as modality
    pub network_connect: Vec<u16>,
    pub max_allowed_uids: u16,   // Same as max_n
    pub registered_neurons: u16, // Same as subnetwork_n
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NeuronInfo {
    pub hotkey: AccountId32,
    pub coldkey: AccountId32,
    pub uid: u16,
    pub netuid: u16,
    pub active: bool,
    pub axon_info: AxonInfo,
    pub prometheus_info: PrometheusInfo,
    pub stake: Vec<(AccountId32, u64)>,
    pub rank: u16,
    pub emission: u64,
    pub incentive: u16,
    pub consensus: u16,
    pub trust: u16,
    pub validator_trust: u16,
    pub dividends: u16,
    pub last_update: u64,
    pub validator_permit: bool,
    pub weights: Vec<(u16, u16)>,
    pub bonds: Vec<(u16, u16)>,
    pub pruning_score: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AxonInfo {
    block: u64,
    version: u32,
    ip: u128,
    port: u16,
    ip_type: u8,
    protocol: u8,
    placeholder1: u8,
    placeholder2: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrometheusInfo {
    block: u64,
    version: u32,
    ip: u128,
    port: u16,
    ip_type: u8,
}

impl Default for AxonInfo {
    fn default() -> Self {
        Self {
            block: 0,
            version: 0,
            ip: 0,
            port: 0,
            ip_type: 0,
            protocol: 0,
            placeholder1: 0,
            placeholder2: 0,
        }
    }
}

impl Default for PrometheusInfo {
    fn default() -> Self {
        Self {
            block: 0,
            version: 0,
            ip: 0,
            port: 0,
            ip_type: 0,
        }
    }
}

#[derive(Debug, Decode)]
struct AccountInfo {
    nonce: u32,
    consumers: u32,
    providers: u32,
    sufficients: u32,
    data: AccountData,
}

// ExtraFlags struct for account flags (removed as not needed for original structure)

#[derive(Debug, Decode, Encode)]
struct AccountData {
    free: u128,
    reserved: u128,
    frozen: u128, // Modern Substrate uses "frozen" instead of misc_frozen/fee_frozen
    flags: u128,  // ExtraFlags - additional account metadata
}

pub struct BittensorClient {
    client: jsonrpsee::ws_client::WsClient,
    endpoint: String,
}

impl BittensorClient {
    pub async fn new(endpoint: String) -> Result<Self> {
        println!("🔗 Connecting to Bittensor network: {}", endpoint);

        let client = WsClientBuilder::default()
            .connection_timeout(Duration::from_secs(30))
            .request_timeout(Duration::from_secs(60))
            .build(&endpoint)
            .await
            .context("Failed to connect to Bittensor RPC endpoint")?;

        println!("✅ Connected to Bittensor network");

        Ok(Self { client, endpoint })
    }

    // Getting subnet information
    pub async fn get_subnet_info(&self, netuid: u16, show_info: bool) -> Result<SubnetInfo> {
        if show_info {
            println!(
                "🔍 Fetching subnet {} information from blockchain...",
                netuid
            );
        }

        // Debug: Let's see what the storage key looks like
        let storage_key: String = self.encode_bittensor_storage_key("SubnetworkN", &[netuid]);
        //println!("🐛 DEBUG: Storage key for SubnetworkN[{}]: {}", netuid, storage_key);

        // Try to get network parameters. If any core parameter doesn't exist, subnet doesn't exist.
        // Start with SubnetworkN which should exist for any active subnet
        let subnetwork_n_raw = self.get_bittensor_storage("SubnetworkN", &[netuid]).await?;
        //println!("🐛 DEBUG: Raw storage result: {:?}", subnetwork_n_raw);

        if subnetwork_n_raw.is_none() {
            // Let's also try to get the total subnet count to see if we can get any storage at all
            let total_networks_key = self.encode_bittensor_storage_key("TotalNetworks", &[]);
            //println!("🐛 DEBUG: Trying TotalNetworks storage key: {}", total_networks_key);
            let total_networks = self.get_bittensor_storage("TotalNetworks", &[]).await?;
            //println!("🐛 DEBUG: TotalNetworks result: {:?}", total_networks);

            if let Some(total_bytes) = total_networks {
                let total = u16::from_le_bytes([total_bytes[0], total_bytes[1]]);
                return Err(anyhow!(
                    "Network {} does not exist. Total networks on chain: {}. Try checking which specific subnet IDs are active.",
                    netuid, total
                ));
            }

            return Err(anyhow!("Network {} does not exist", netuid));
        }

        let subnetwork_n = u16::from_le_bytes([
            subnetwork_n_raw.as_ref().unwrap()[0],
            subnetwork_n_raw.as_ref().unwrap()[1],
        ]);

        if subnetwork_n == 0 {
            return Err(anyhow!("Network {} does not exist (has 0 neurons)", netuid));
        }

        // Get network parameters using correct Bittensor storage keys
        let difficulty = self.get_bittensor_u256("Difficulty", &[netuid]).await?;
        let tempo = self.get_bittensor_u16("Tempo", &[netuid]).await?;
        let immunity_period = self.get_bittensor_u16("ImmunityPeriod", &[netuid]).await?;
        let min_allowed_weights = self
            .get_bittensor_u16("MinAllowedWeights", &[netuid])
            .await?;
        let max_weight_limit = self.get_bittensor_u16("MaxWeightsLimit", &[netuid]).await?;
        let max_allowed_validators = self
            .get_bittensor_u16("MaxAllowedValidators", &[netuid])
            .await?;
        let max_n = self.get_bittensor_u16("MaxAllowedUids", &[netuid]).await?;
        let burn = self.get_bittensor_u64("Burn", &[netuid]).await?;
        let owner_account = self.get_bittensor_account("SubnetOwner", &[netuid]).await?;
        let owner_ss58 = owner_account.to_ss58check();
        let modality = self.get_bittensor_u16("NetworkModality", &[netuid]).await?;
        let emission_value = self.get_bittensor_u64("EmissionValues", &[netuid]).await?;
        let rho = self.get_bittensor_u16("Rho", &[netuid]).await?;
        let kappa = self.get_bittensor_u16("Kappa", &[netuid]).await?;
        let scaling_law_power = self.get_bittensor_u16("ScalingLawPower", &[netuid]).await?;
        let blocks_since_epoch = self
            .get_bittensor_u64("BlocksSinceLastStep", &[netuid])
            .await?;

        let current_block = self.get_current_block().await?;

        if show_info {
            println!("📋 Subnet {} info retrieved:", netuid);
            println!("   Difficulty: {}", difficulty);
            println!("   Tempo: {}", tempo);
            println!("   Immunity period: {}", immunity_period);
            println!("   Min allowed weights: {}", min_allowed_weights);
            println!("   Registration burn: {} RAO", burn);
            println!("   Registered neurons: {}", subnetwork_n);
            println!("   Current block: {}", current_block);
            println!("   Owner: {}", owner_ss58);
            println!("🐛 DEBUG: Full owner address: {}", owner_ss58);

            // Debug: Test account info with the subnet owner (known to exist)
            println!("🐛 DEBUG: Testing account info with subnet owner...");
            if let Err(e) = self.debug_account_info(&owner_account).await {
                println!("🐛 DEBUG: Account info test failed: {}", e);
            }
        }

        Ok(SubnetInfo {
            netuid,
            difficulty,
            immunity_period,
            min_allowed_weights,
            max_weight_limit,
            max_allowed_validators,
            max_n,
            tempo,
            burn,
            owner_ss58,
            emission_value,
            rho,
            kappa,
            scaling_law_power,
            subnetwork_n,
            blocks_since_epoch,
            modality,
            // Legacy fields for backward compatibility
            network_modality: modality,
            network_connect: vec![],
            max_allowed_uids: max_n,
            registered_neurons: subnetwork_n,
        })
    }

    // Bittensor-specific storage key generation
    fn encode_bittensor_storage_key(&self, storage_name: &str, keys: &[u16]) -> String {
        use sp_core::twox_128;

        // Bittensor uses "SubtensorModule" as the pallet name
        let pallet_hash = twox_128(b"SubtensorModule");
        let storage_hash = twox_128(storage_name.as_bytes());

        let mut final_key = Vec::new();
        final_key.extend_from_slice(&pallet_hash);
        final_key.extend_from_slice(&storage_hash);

        // For map storage items with Identity hasher, use the key directly (no hashing)
        // SubnetworkN uses Identity hasher according to the source code
        if !keys.is_empty() {
            for &key in keys {
                // NetUid is u16, encode as little-endian bytes
                final_key.extend_from_slice(&key.to_le_bytes());
            }
        }

        format!("0x{}", hex::encode(final_key))
    }

    // Get raw storage data from Bittensor
    async fn get_bittensor_storage(
        &self,
        storage_name: &str,
        keys: &[u16],
    ) -> Result<Option<Vec<u8>>> {
        let storage_key = self.encode_bittensor_storage_key(storage_name, keys);

        let result: Option<String> = self
            .client
            .request("state_getStorage", rpc_params![storage_key])
            .await
            .context(format!(
                "Failed to get {} from SubtensorModule",
                storage_name
            ))?;

        if let Some(hex_data) = result {
            let bytes = hex::decode(&hex_data[2..]).context("Invalid hex data in storage")?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    // Get and decode storage data from Bittensor
    async fn get_bittensor_storage_decoded<T>(&self, storage_name: &str, keys: &[u16]) -> Result<T>
    where
        T: codec::Decode,
    {
        if let Some(bytes) = self.get_bittensor_storage(storage_name, keys).await? {
            let value = T::decode(&mut &bytes[..])
                .map_err(|e| anyhow!("Failed to decode {}: {:?}", storage_name, e))?;
            Ok(value)
        } else {
            Err(anyhow!("Storage key not found: {}", storage_name))
        }
    }

    // Specialized getters for different types
    async fn get_bittensor_u16(&self, storage_name: &str, keys: &[u16]) -> Result<u16> {
        self.get_bittensor_storage_decoded(storage_name, keys)
            .await
            .or_else(|_| Ok(0u16))
    }

    async fn get_bittensor_u64(&self, storage_name: &str, keys: &[u16]) -> Result<u64> {
        self.get_bittensor_storage_decoded(storage_name, keys)
            .await
            .or_else(|_| Ok(0u64))
    }

    async fn get_bittensor_u256(&self, storage_name: &str, keys: &[u16]) -> Result<U256> {
        self.get_bittensor_storage_decoded(storage_name, keys)
            .await
            .or_else(|_| Ok(U256::zero()))
    }

    async fn get_bittensor_account(&self, storage_name: &str, keys: &[u16]) -> Result<AccountId32> {
        self.get_bittensor_storage_decoded(storage_name, keys)
            .await
            .or_else(|_| Ok(AccountId32::new([0u8; 32])))
    }

    // Specialized method for account-based storage keys
    async fn get_bittensor_storage_with_account(
        &self,
        storage_name: &str,
        netuid: u16,
        account: &AccountId32,
    ) -> Result<Option<Vec<u8>>> {
        use sp_core::{blake2_256, twox_128};

        let pallet_hash = twox_128(b"SubtensorModule");
        let storage_hash = twox_128(storage_name.as_bytes());

        let mut final_key = Vec::new();
        final_key.extend_from_slice(&pallet_hash);
        final_key.extend_from_slice(&storage_hash);

        // Create the composite key for double map (netuid, account)
        let mut map_key = Vec::new();
        map_key.extend_from_slice(&netuid.to_le_bytes());
        map_key.extend_from_slice(account.as_ref());

        let key_hash = blake2_256(&map_key);
        final_key.extend_from_slice(&key_hash);

        let storage_key = format!("0x{}", hex::encode(final_key));

        let result: Option<String> = self
            .client
            .request("state_getStorage", rpc_params![storage_key])
            .await
            .context(format!(
                "Failed to get {} from SubtensorModule",
                storage_name
            ))?;

        if let Some(hex_data) = result {
            let bytes = hex::decode(&hex_data[2..]).context("Invalid hex data in storage")?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    // Helper method to encode storage keys (legacy - keep for compatibility)
    fn encode_storage_key(&self, _module: &str, storage: &str, keys: &[u16]) -> Result<String> {
        // Use the new Bittensor-specific method
        Ok(self.encode_bittensor_storage_key(storage, keys))
    }

    // Generic storage value getter
    async fn get_storage_value<T>(&self, module: &str, storage: &str, keys: &[u16]) -> Result<T>
    where
        T: codec::Decode + Default,
    {
        let storage_key = self.encode_storage_key(module, storage, keys)?;

        let result: Option<String> = self
            .client
            .request("state_getStorage", rpc_params![storage_key])
            .await
            .context(format!("Failed to get {} from {}", storage, module))?;

        if let Some(hex_data) = result {
            let bytes = hex::decode(&hex_data[2..]).context("Invalid hex data in storage")?;
            let value = T::decode(&mut &bytes[..])
                .map_err(|_| anyhow!("Failed to decode storage value"))?;
            Ok(value)
        } else {
            Ok(T::default())
        }
    }

    // Raw storage getter for custom decoding
    async fn get_storage_raw(&self, module: &str, storage: &str, keys: &[u16]) -> Result<Vec<u8>> {
        let storage_key = self.encode_storage_key(module, storage, keys)?;

        let result: Option<String> = self
            .client
            .request("state_getStorage", rpc_params![storage_key])
            .await
            .context(format!("Failed to get {} from {}", storage, module))?;

        if let Some(hex_data) = result {
            let bytes = hex::decode(&hex_data[2..]).context("Invalid hex data in storage")?;
            Ok(bytes)
        } else {
            Err(anyhow!("Storage key not found"))
        }
    }

    // Checking neuron registration
    pub async fn check_registration(
        &self,
        netuid: u16,
        hotkey: &AccountId32,
    ) -> Result<Option<NeuronInfo>> {
        println!("🔍 Checking registration status for hotkey: {}", hotkey);

        // Get UID for hotkey using Bittensor storage
        let uid_data = self
            .get_bittensor_storage_with_account("Uids", netuid, hotkey)
            .await?;

        let uid = match uid_data {
            Some(bytes) if bytes.len() >= 2 => u16::from_le_bytes([bytes[0], bytes[1]]),
            _ => {
                println!("❌ Hotkey not registered in subnet {}", netuid);
                return Ok(None);
            }
        };

        // Get neuron info using UID - this requires a different storage key format
        let neuron_data = self
            .get_bittensor_storage("Neurons", &[netuid, uid])
            .await?;

        match neuron_data {
            Some(bytes) => {
                // For now, create a simplified neuron info since full decoding is complex
                // In a real implementation, you'd need to properly decode the neuron struct
                let neuron_info = NeuronInfo {
                    hotkey: hotkey.clone(),
                    coldkey: AccountId32::new([0u8; 32]), // Would need proper decoding
                    uid,
                    netuid,
                    active: true,
                    axon_info: AxonInfo::default(),
                    prometheus_info: PrometheusInfo::default(),
                    stake: vec![],
                    rank: 0,
                    emission: 0,
                    incentive: 0,
                    consensus: 0,
                    trust: 0,
                    validator_trust: 0,
                    dividends: 0,
                    last_update: 0,
                    validator_permit: false,
                    weights: vec![],
                    bonds: vec![],
                    pruning_score: 0,
                };

                println!("✅ Neuron registered:");
                println!("   UID: {}", uid);
                println!("   Hotkey: {}", hotkey);
                println!("   Active: {}", neuron_info.active);
                println!("   Raw data length: {} bytes", bytes.len());
                Ok(Some(neuron_info))
            }
            None => {
                println!("❌ No neuron data found for UID {}", uid);
                Ok(None)
            }
        }
    }

    // Helper method to encode storage keys with hotkey
    fn encode_hotkey_storage_key(
        &self,
        module: &str,
        storage: &str,
        netuid: u16,
        hotkey: &AccountId32,
    ) -> Result<String> {
        use sp_core::blake2_256;

        let module_hash = blake2_256(module.as_bytes());
        let storage_hash = blake2_256(storage.as_bytes());

        let mut key = Vec::new();
        key.extend_from_slice(&module_hash);
        key.extend_from_slice(&storage_hash);
        key.extend_from_slice(&netuid.to_le_bytes());
        key.extend_from_slice(hotkey.as_ref());

        Ok(format!("0x{}", hex::encode(key)))
    }

    // Helper method to encode storage keys with UID
    fn encode_uid_storage_key(
        &self,
        module: &str,
        storage: &str,
        netuid: u16,
        uid: u16,
    ) -> Result<String> {
        use sp_core::blake2_256;

        let module_hash = blake2_256(module.as_bytes());
        let storage_hash = blake2_256(storage.as_bytes());

        let mut key = Vec::new();
        key.extend_from_slice(&module_hash);
        key.extend_from_slice(&storage_hash);
        key.extend_from_slice(&netuid.to_le_bytes());
        key.extend_from_slice(&uid.to_le_bytes());

        Ok(format!("0x{}", hex::encode(key)))
    }

    // Getting current block number
    pub async fn get_current_block(&self) -> Result<u64> {
        let block_hash: H256 = self
            .client
            .request("chain_getBlockHash", rpc_params![])
            .await
            .context("Failed to get current block hash")?;

        let header: serde_json::Value = self
            .client
            .request("chain_getHeader", rpc_params![block_hash])
            .await
            .context("Failed to get block header")?;

        let block_number = header["number"]
            .as_str()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .ok_or_else(|| anyhow!("Invalid block number format"))?;

        Ok(block_number)
    }

    // Creating a signed extrinsic
    async fn create_signed_extrinsic(
        &self,
        call: Vec<u8>,
        signer: &Sr25519Pair,
    ) -> Result<Vec<u8>> {
        let account_id = AccountId32::from(signer.public().0);
        let account_info = self.get_account_info(&account_id).await?;
        let current_block = self.get_current_block().await?;

        // Getting genesis hash and current block hash
        let _genesis_hash = self.get_genesis_hash().await?;
        let _block_hash = self.get_block_hash(None).await?;

        // Creating signed extra
        let extra = self.create_signed_extra(account_info.nonce as u64, current_block)?;

        // Creating payload for signing
        let mut payload = Vec::new();
        call.encode_to(&mut payload);
        extra.encode_to(&mut payload);

        // If payload is more than 256 bytes, hash it
        let signing_payload = if payload.len() > 256 {
            sp_core::blake2_256(&payload).to_vec()
        } else {
            payload
        };

        // Signing
        let signature = signer.sign(&signing_payload);

        // Creating extrinsic
        let mut extrinsic = Vec::new();

        // Version и signature type
        extrinsic.push(0x84u8); // Version 4 with signature

        // Signer
        account_id.encode_to(&mut extrinsic);

        // Signature
        signature.encode_to(&mut extrinsic);

        // Extra
        extra.encode_to(&mut extrinsic);

        // Call
        call.encode_to(&mut extrinsic);

        // Add length prefix
        let mut final_extrinsic = Vec::new();
        ((extrinsic.len() as u32) | 0x8000_0000).encode_to(&mut final_extrinsic);
        final_extrinsic.extend(extrinsic);

        Ok(final_extrinsic)
    }

    fn create_signed_extra(&self, nonce: u64, block_number: u64) -> Result<Vec<u8>> {
        let mut extra = Vec::new();

        // Era (mortal)
        let era_period = 64u64;
        let phase = block_number % era_period;
        let era = ((era_period.trailing_zeros() - 1).max(1) as u8)
            | ((phase / (era_period >> 4)) as u8) << 6;
        extra.push(era);
        extra.push(0u8);

        // Nonce
        nonce.encode_to(&mut extra);

        // Tip
        0u64.encode_to(&mut extra); // No tip

        Ok(extra)
    }

    async fn get_genesis_hash(&self) -> Result<H256> {
        let result: String = self
            .client
            .request("chain_getBlockHash", rpc_params![0])
            .await
            .context("Failed to get genesis hash")?;

        Ok(H256::from_str(&result[2..])?)
    }

    async fn get_block_hash(&self, block_number: Option<u64>) -> Result<H256> {
        let params = if let Some(block) = block_number {
            rpc_params![block]
        } else {
            rpc_params![]
        };

        let result: String = self
            .client
            .request("chain_getBlockHash", params)
            .await
            .context("Failed to get block hash")?;

        Ok(H256::from_str(&result[2..])?)
    }

    async fn submit_extrinsic(&self, extrinsic: String) -> Result<H256> {
        let result: String = self
            .client
            .request(
                "author_submitExtrinsic",
                rpc_params![format!("0x{}", extrinsic)],
            )
            .await
            .context("Failed to submit extrinsic")?;

        Ok(H256::from_str(&result[2..])?)
    }

    // Getting burn registration cost
    /*async fn get_burn_cost(&self, netuid: u16) -> Result<u64> {
        let params = rpc_params![
            "SubtensorModule",
            "Burn",
            format!("0x{}", netuid.to_be_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>())
        ];

        let result: Option<String> = self.client
            .request("state_getStorage", params)
            .await
            .context("Failed to get burn cost")?;

        if let Some(hex_data) = result {
            let bytes = hex::decode(&hex_data[2..])
                .context("Invalid hex data")?;
            let burn_cost = u64::from_le_bytes(
                bytes.try_into()
                    .map_err(|_| anyhow!("Invalid burn cost data"))?
            );
            Ok(burn_cost)
        } else {
            // Default burn cost if not set
            Ok(1_000_000_000) // 1 TAO in RAO
        }
    }*/

    // Getting account balance
    pub async fn get_account_balance(&self, account: &AccountId32) -> Result<u64> {
        let account_info = self.get_account_info(account).await?;
        Ok(account_info.data.free as u64)
    }

    async fn get_account_info(&self, account: &AccountId32) -> Result<AccountInfo> {
        // Create storage key for System::Account
        let storage_key = self.encode_system_account_storage_key(account);
        let result: Option<String> = match self
            .client
            .request("state_getStorage", rpc_params![storage_key])
            .await
        {
            Ok(res) => res,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get account info: {}", e));
            }
        };

        if let Some(hex_data) = result {
            let bytes = hex::decode(&hex_data[2..]).context("Invalid hex data in account info")?;

            // Use proper SCALE decoding
            match AccountInfo::decode(&mut &bytes[..]) {
                Ok(account_info) => Ok(account_info),
                Err(e) => {
                    // Manual parsing following Python Bittensor approach
                    // AccountInfo structure: nonce(4) + consumers(4) + providers(4) + sufficients(4) + AccountData(40)
                    // AccountData structure: free(16) + reserved(16) + frozen(8) + flags(8) = 48 bytes
                    // But we're seeing 56 bytes total, so AccountData is actually 40 bytes: free(16) + reserved(16) + frozen(8)
                    if bytes.len() >= 56 {
                        // Parse AccountInfo fields (first 16 bytes)
                        let nonce = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                        let consumers =
                            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                        let providers =
                            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                        let sufficients =
                            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

                        // Parse AccountData fields (next 40 bytes, starting at byte 16)
                        // AccountData: free(16) + reserved(16) + misc_frozen(8) + fee_frozen(8) = 48 bytes
                        // But actual data shows 40 bytes, so structure might be: free(16) + reserved(16) + frozen_data(8)

                        // Free balance (bytes 16-31: 16 bytes for u128)
                        let free_bytes = &bytes[16..32];
                        let free = u128::from_le_bytes([
                            free_bytes[0],
                            free_bytes[1],
                            free_bytes[2],
                            free_bytes[3],
                            free_bytes[4],
                            free_bytes[5],
                            free_bytes[6],
                            free_bytes[7],
                            free_bytes[8],
                            free_bytes[9],
                            free_bytes[10],
                            free_bytes[11],
                            free_bytes[12],
                            free_bytes[13],
                            free_bytes[14],
                            free_bytes[15],
                        ]);

                        // Reserved balance (bytes 32-47: 16 bytes for u128)
                        let reserved = if bytes.len() >= 48 {
                            let reserved_bytes = &bytes[32..48];
                            u128::from_le_bytes([
                                reserved_bytes[0],
                                reserved_bytes[1],
                                reserved_bytes[2],
                                reserved_bytes[3],
                                reserved_bytes[4],
                                reserved_bytes[5],
                                reserved_bytes[6],
                                reserved_bytes[7],
                                reserved_bytes[8],
                                reserved_bytes[9],
                                reserved_bytes[10],
                                reserved_bytes[11],
                                reserved_bytes[12],
                                reserved_bytes[13],
                                reserved_bytes[14],
                                reserved_bytes[15],
                            ])
                        } else {
                            0u128
                        };

                        // Frozen balances - in modern Substrate this is a single "frozen" field
                        // and flags field (ExtraFlags) - let's parse what we have
                        let (frozen, flags) = if bytes.len() >= 56 {
                            // The remaining 8 bytes might be compressed or represent flags
                            // Try to parse as single u64 frozen amount
                            let remaining_bytes = &bytes[48..56];
                            let frozen_u64 = u64::from_le_bytes([
                                remaining_bytes[0],
                                remaining_bytes[1],
                                remaining_bytes[2],
                                remaining_bytes[3],
                                remaining_bytes[4],
                                remaining_bytes[5],
                                remaining_bytes[6],
                                remaining_bytes[7],
                            ]);

                            // Convert to u128 for consistency
                            let frozen = frozen_u64 as u128;
                            let flags = 0u128; // Default flags

                            (frozen, flags)
                        } else {
                            (0u128, 0u128)
                        };

                        Ok(AccountInfo {
                            nonce,
                            consumers,
                            providers,
                            sufficients,
                            data: AccountData {
                                free,
                                reserved,
                                frozen,
                                flags,
                            },
                        })
                    } else {
                        Ok(AccountInfo {
                            nonce: 0,
                            consumers: 0,
                            providers: 0,
                            sufficients: 0,
                            data: AccountData {
                                free: 0,
                                reserved: 0,
                                frozen: 0,
                                flags: 0,
                            },
                        })
                    }
                }
            }
        } else {
            // Account doesn't exist
            Ok(AccountInfo {
                nonce: 0,
                consumers: 0,
                providers: 0,
                sufficients: 0,
                data: AccountData {
                    free: 0,
                    reserved: 0,
                    frozen: 0,
                    flags: 0,
                },
            })
        }
    }

    // Helper function to encode System::Account storage key
    fn encode_system_account_storage_key(&self, account: &AccountId32) -> String {
        use sp_core::{blake2_128, twox_128};

        // System pallet hash
        let pallet_hash = twox_128(b"System");

        // Account storage hash
        let storage_hash = twox_128(b"Account");

        // For System::Account, Substrate uses Blake2_128Concat hasher
        // This means: blake2_128(key) + key
        let account_hash = blake2_128(account.as_ref());

        let mut final_key = Vec::new();
        final_key.extend_from_slice(&pallet_hash); // 16 bytes
        final_key.extend_from_slice(&storage_hash); // 16 bytes
        final_key.extend_from_slice(&account_hash); // 16 bytes
        final_key.extend_from_slice(account.as_ref()); // 32 bytes

        format!("0x{}", hex::encode(final_key))
    }

    // Debug function to test account info with known accounts
    pub async fn debug_account_info(&self, account: &AccountId32) -> Result<()> {
        println!(
            "🐛 DEBUG: Testing account info for: {}",
            account.to_ss58check()
        );

        // First test the raw storage access
        let storage_key = self.encode_system_account_storage_key(account);
        println!("🐛 DEBUG: Storage key: {}", storage_key);
        println!(
            "🐛 DEBUG: Storage key length: {} bytes",
            (storage_key.len() - 2) / 2
        );

        let result: Option<String> = self
            .client
            .request("state_getStorage", rpc_params![storage_key])
            .await
            .context("Failed to get account info")?;

        println!("🐛 DEBUG: Raw result: {:?}", result);

        if let Some(hex_data) = result {
            println!("🐛 DEBUG: Hex data: {}", hex_data);
            println!("🐛 DEBUG: Data length: {} bytes", (hex_data.len() - 2) / 2);

            let bytes = hex::decode(&hex_data[2..]).context("Invalid hex data")?;
            println!("🐛 DEBUG: Decoded bytes length: {}", bytes.len());
            println!(
                "🐛 DEBUG: First 20 bytes: {:?}",
                &bytes[..20.min(bytes.len())]
            );

            // Now test the account info parsing
            println!("🐛 DEBUG: Testing get_account_info parsing...");
            match self.get_account_info(account).await {
                Ok(account_info) => {
                    println!("🐛 DEBUG: ✅ Account info parsed successfully!");
                    println!("🐛 DEBUG: Nonce: {}", account_info.nonce);
                    println!("🐛 DEBUG: Consumers: {}", account_info.consumers);
                    println!("🐛 DEBUG: Providers: {}", account_info.providers);
                    println!("🐛 DEBUG: Sufficients: {}", account_info.sufficients);
                    println!("🐛 DEBUG: Free balance: {} RAO", account_info.data.free);
                    println!(
                        "🐛 DEBUG: Free balance: {:.6} TAO",
                        utils::format_tao(account_info.data.free)
                    );
                    println!("🐛 DEBUG: Reserved: {} RAO", account_info.data.reserved);
                    println!("🐛 DEBUG: Frozen balance: {} RAO", account_info.data.frozen);
                    println!("🐛 DEBUG: Flags: {:#x}", account_info.data.flags);
                }
                Err(e) => {
                    println!("🐛 DEBUG: ❌ Failed to parse account info: {}", e);
                }
            }
        } else {
            println!("🐛 DEBUG: No data returned - account doesn't exist or wrong storage key");
        }

        Ok(())
    }

    // Sending burned registration
    pub async fn submit_burned_registration(
        &self,
        registration_data: &RegistrationData,
        signer: &Sr25519Pair,
    ) -> Result<H256> {
        println!("🔥 Submitting burned registration transaction...");

        // Creating extrinsic for burned registration
        let call = self.encode_burned_register_call(
            registration_data.subnet_id,
            registration_data.hotkey.clone(),
            registration_data.burn_amount,
        )?;

        let extrinsic = self.create_signed_extrinsic(call, signer).await?;
        self.submit_extrinsic(hex::encode(extrinsic)).await
    }

    // Encoding burned register call
    fn encode_burned_register_call(
        &self,
        netuid: u16,
        hotkey: AccountId32,
        burn_amount: u64,
    ) -> Result<Vec<u8>> {
        let mut call = Vec::new();

        // Module index (SubtensorModule)
        call.push(8u8);

        // Call index (burned_register)
        call.push(1u8);

        // Parameters
        netuid.encode_to(&mut call);
        hotkey.encode_to(&mut call);
        burn_amount.encode_to(&mut call);

        Ok(call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_data_encode() {
        let registration = RegistrationData {
            subnet_id: 1,
            hotkey: AccountId32::new([1u8; 32]),
            coldkey: AccountId32::new([2u8; 32]),
            burn_amount: 12345,
            block_number: 67890,
        };

        // Serialization test
        let json = serde_json::to_string(&registration);
        assert!(json.is_ok());
    }
}
