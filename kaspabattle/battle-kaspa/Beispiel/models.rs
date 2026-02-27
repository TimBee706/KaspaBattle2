

use serde::{Deserialize, Serialize};
use kaspa_wallet_core::prelude::*;
use kaspa_wrpc_client::KaspaRpcClient;
use kaspa_consensus_core::network::NetworkId;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, serde::Serialize)]
pub struct WalletInfo {
    pub id: String,
    pub name: Option<String>,
    pub mnemonic: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub node_connected: bool,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub available: u64,
    pub total: u64,
}

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    pub mnemonic: Option<String>,
}

#[derive(Serialize)]
pub struct CreateAccountResponse {
    pub status: String,
    pub mnemonic: String,
}

#[derive(Serialize)]
pub struct MnemonicResponse {
    pub mnemonic: String,
}

#[derive(Deserialize)]
pub struct CreateTimelockRequest {
    pub destination: String,
    pub amount: u64,
    pub locktime: u64,
}

#[derive(Serialize)]
pub struct TimelockResponse {
    pub tx_id: String,
    pub locktime: u64,
    pub raw_tx: String,
    pub status: String,
}

// ===== KASPA WALLET STRUKTUR =====

pub struct KaspaWallet {
    #[allow(dead_code)]
    pub(crate) wallet: Arc<Wallet>,
    pub(crate) account_id: Arc<Mutex<Option<AccountId>>>,
    #[allow(dead_code)]
    pub(crate) rpc_client: Arc<KaspaRpcClient>,
    #[allow(dead_code)]
    pub(crate) network_id: NetworkId,
    pub(crate) mnemonic: Arc<Mutex<Option<String>>>,
}




