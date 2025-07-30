use jsonrpsee::{core::client::ClientT, rpc_params, ws_client::WsClientBuilder};
use primitive_types::{H256, U256};
use codec::Encode;
use serde::{Deserialize, Serialize};
use sp_core::{
    crypto::{AccountId32, Ss58Codec},
    sr25519::{Pair as Sr25519Pair},
    Pair,
};
use std::{
    str::FromStr,
    time::Duration,
};
use anyhow::{anyhow, Context, Result};

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
    pub difficulty: U256,
    pub immunity_period: u16,
    pub min_allowed_weights: u16,
    pub max_weight_limit: u16,
    pub tempo: u16,
    pub burn: u64, // Burned registration cost in RAO
    pub owner: AccountId32,
    pub max_allowed_uids: u16,
    pub network_modality: u16,
    pub network_connect: Vec<u16>,
    pub emission_values: u64,
    pub registered_neurons: u16, // Текущее количество зарегистрированных нейронов
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

#[derive(Debug, Deserialize)]
struct AccountInfo {
    data: AccountData,
    nonce: u64,
}

#[derive(Debug, Deserialize)]
struct AccountData {
    free: u64,
    reserved: u64,
    frozen: u64,
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
    pub async fn get_subnet_info(&self, netuid: u16) -> Result<SubnetInfo> {
        println!("🔍 Fetching subnet {} information from blockchain...", netuid);

        // Getting network difficulty
        let difficulty: Option<U256> = self.client
            .request("subtensorModule_difficulty", rpc_params![netuid])
            .await
            .context("Failed to get network difficulty")?;
 
        let difficulty = difficulty.ok_or_else(|| anyhow!("Network {} does not exist", netuid))?;

        // Getting other network parameters
        let tempo: u16 = self.client
            .request("subtensorModule_tempo", rpc_params![netuid])
            .await
            .unwrap_or(99);
 
        let immunity_period: u16 = self.client
            .request("subtensorModule_immunityPeriod", rpc_params![netuid])
            .await
            .unwrap_or(7200);
 
        let min_allowed_weights: u16 = self.client
            .request("subtensorModule_minAllowedWeights", rpc_params![netuid])
            .await
            .unwrap_or(8);
 
        let max_weight_limit: u16 = self.client
            .request("subtensorModule_maxWeightLimit", rpc_params![netuid])
            .await
            .unwrap_or(1000);
 
        let burn: u64 = self.client
            .request("subtensorModule_burn", rpc_params![netuid])
            .await
            .unwrap_or(1_000_000_000); // 1 TAO в rao
 
        let owner: AccountId32 = self.client
            .request("subtensorModule_subnetOwner", rpc_params![netuid])
            .await
            .unwrap_or_else(|_| AccountId32::new([0u8; 32]));
 
        println!("📋 Subnet {} info retrieved:", netuid);
        println!("   Difficulty: {}", difficulty);
        println!("   Tempo: {}", tempo);
        println!("   Immunity period: {}", immunity_period);
        println!("   Min allowed weights: {}", min_allowed_weights);
        println!("   Registration burn: {} RAO", burn);
 
        Ok(SubnetInfo {
            difficulty,
            immunity_period,
            min_allowed_weights,
            max_weight_limit,
            tempo,
            burn,
            owner,
            max_allowed_uids: netuid,
            network_modality: 0,
            network_connect: vec![],
            emission_values: 0,
            registered_neurons: 0, // Current number of registered neurons
        })
    }

    // Checking neuron registration
    pub async fn check_registration(&self, netuid: u16, hotkey: &AccountId32) -> Result<Option<NeuronInfo>> {
        println!("🔍 Checking registration status for hotkey: {}", hotkey);

        // Getting neuron UID by hotkey
        let uid: Option<u16> = self.client
            .request("subtensorModule_uids", rpc_params![netuid, hotkey])
            .await
            .context("Failed to query neuron UID")?;
 
        let uid = match uid {
            Some(uid) => uid,
            None => {
                println!("❌ Hotkey not registered in subnet {}", netuid);
                return Ok(None);
            }
        };
 
        // Getting detailed neuron information
        let neuron_info: Option<NeuronInfo> = self.client
            .request("subtensorModule_neurons", rpc_params![netuid, uid])
            .await
            .context("Failed to get neuron info")?;
 
        match neuron_info {
            Some(info) => {
                println!("✅ Neuron registered:");
                println!("   UID: {}", uid);
                println!("   Hotkey: {}", info.hotkey);
                println!("   Coldkey: {}", info.coldkey);
                println!("   Active: {}", info.active);
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    // Getting current block number
    pub async fn get_current_block(&self) -> Result<u64> {
        let block_hash: H256 = self.client
            .request("chain_getBlockHash", rpc_params![])
            .await
            .context("Failed to get current block hash")?;
 
        let header: serde_json::Value = self.client
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
        let extra = self.create_signed_extra(account_info.nonce, current_block)?;
        
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
        
        // Добавляем длину в начало
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
        let era = ((era_period.trailing_zeros() - 1).max(1) as u8) | ((phase / (era_period >> 4)) as u8) << 6;
        extra.push(era);
        extra.push(0u8);
        
        // Nonce
        nonce.encode_to(&mut extra);
        
        // Tip
        0u64.encode_to(&mut extra); // No tip
        
        Ok(extra)
    }
 
    async fn get_genesis_hash(&self) -> Result<H256> {
        let result: String = self.client
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
        
        let result: String = self.client
            .request("chain_getBlockHash", params)
            .await
            .context("Failed to get block hash")?;
            
        Ok(H256::from_str(&result[2..])?)
    }
    
    async fn submit_extrinsic(&self, extrinsic: String) -> Result<H256> {
        let result: String = self.client
            .request("author_submitExtrinsic", rpc_params![format!("0x{}", extrinsic)])
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
        Ok(account_info.data.free)
    }
    
    async fn get_account_info(&self, account: &AccountId32) -> Result<AccountInfo> {
        let params = rpc_params![
            "System",
            "Account", 
            account.to_ss58check()
        ];
        
        let result: Option<serde_json::Value> = self.client
            .request("state_getStorage", params)
            .await
            .context("Failed to get account info")?;
            
        if let Some(data) = result {
            // Parsing account data (simplified version)
            Ok(AccountInfo {
                data: AccountData {
                    free: data.get("free")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                    reserved: 0,
                    frozen: 0,
                },
                nonce: data.get("nonce")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
            })
        } else {
            Ok(AccountInfo {
                data: AccountData { free: 0, reserved: 0, frozen: 0 },
                nonce: 0,
            })
        }
    }

    // Sending burned registration
    pub async fn submit_burned_registration(
        &self, 
        registration_data: &RegistrationData,
        signer: &Sr25519Pair
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
 
        // Тест сериализации
        let json = serde_json::to_string(&registration);
        assert!(json.is_ok());
    }
}
