use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoInfo {
    pub tx_id: String,
    pub output_index: u32,
    pub amount: u64,
    pub amount_kas: f64,
    pub is_coinbase: bool,
    pub block_daa_score: u64,
    pub script_public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub server_version: String,
    pub is_synced: bool,
    pub is_utxo_indexed: bool,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub normal_bucket_feerate: f64,
    pub low_bucket_feerate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum KaspaError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Kaspa node is not synced")]
    NodeNotSynced,
    #[error("UTXO index not enabled (start node with --utxoindex)")]
    UtxoIndexNotEnabled,
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    #[error("Double-spend detected: {0}")]
    DoubleSpend(String),
    #[error("Kaspa mempool is full — retry later")]
    MempoolFull,
    #[error("RPC request timed out")]
    Timeout,
}

/// Abstract traits for Kaspa blockchain interaction.
/// Allows swapping out inner Node RPC bindings (wRPC, gRPC, etc).
#[async_trait]
pub trait KaspaBackend: Send + Sync {
    async fn connect(&self) -> std::result::Result<(), KaspaError>;
    async fn disconnect(&self) -> std::result::Result<(), KaspaError>;
    async fn is_connected(&self) -> bool;
    async fn is_synced(&self) -> std::result::Result<bool, KaspaError>;
    
    async fn get_balance(&self, address: &str) -> std::result::Result<u64, KaspaError>;
    async fn get_utxos(&self, address: &str) -> std::result::Result<Vec<UtxoInfo>, KaspaError>;
    
    /// Abstract transaction submission: Avoids leaking kaspa_rpc_core types.
    /// Expects a fully assembled, serialized Kaspa transaction encoded as a Hex string
    /// or JSON payload. JSON of RpcTransaction is standard.
    async fn submit_transaction(&self, tx_payload: &str) -> std::result::Result<String, KaspaError>;
    
    async fn get_node_info(&self) -> std::result::Result<NodeInfo, KaspaError>;
    async fn get_fee_estimate(&self) -> std::result::Result<FeeEstimate, KaspaError>;
    async fn get_current_daa_score(&self) -> std::result::Result<u64, KaspaError>;
    async fn wait_for_sync(&self, timeout: Duration) -> std::result::Result<(), KaspaError>;
}
