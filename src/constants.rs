//! Additional constants for Bittensor registration

pub const BITTENSOR_SS58_FORMAT: u16 = 42; // Substrate SS58 format
pub const DEFAULT_RPC_ENDPOINTS: &[&str] = &[
    "wss://entrypoint-finney.opentensor.ai:443",
    "wss://archive.chain.opentensor.ai:443",
];
pub const SUBTENSOR_MODULE_INDEX: u8 = 8;
pub const REGISTER_CALL_INDEX: u8 = 0;
pub const BURNED_REGISTER_CALL_INDEX: u8 = 1;
pub const DEFAULT_BLOCK_TIME: u64 = 12; // seconds
pub const TAO_DECIMALS: u32 = 9;
pub const RAO_PER_TAO: u64 = 1_000_000_000;
