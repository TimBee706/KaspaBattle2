/// RPC abstraction layer for Kaspa node communication.
///
/// Defines `KaspaRpc` trait implemented by both `RealKaspaClient` (wRPC)
/// and `MockKaspaClient` (testing). All blockchain-facing code consumes
/// `Arc<dyn KaspaRpc>` so tests can swap in the mock transparently.
///
/// F-003: `submit_transaction` now calls the real Kaspa node RPC instead of
/// always returning an error. `get_fee_estimate` was added to the trait.
use async_trait::async_trait;

use kaspa_rpc_core::model::tx::RpcTransaction;
use serde::Serialize;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

// === Data Types ===

/// Information about a single UTXO.
#[derive(Debug, Clone, Serialize)]
pub struct UtxoInfo {
    pub tx_id: String,
    pub output_index: u32,
    pub amount: u64,
    pub amount_kas: f64,
    pub is_coinbase: bool,
    pub block_daa_score: u64,
    /// The raw script_public_key bytes (hex-encoded)
    pub script_public_key: Option<String>,
}

/// Node status information.
#[derive(Debug, Clone, Serialize)]
pub struct NodeInfo {
    pub server_version: String,
    pub is_synced: bool,
    pub is_utxo_indexed: bool,
    pub network: String,
}

/// Fee estimate from the network (in sompi per mass unit).
#[derive(Debug, Clone, Serialize)]
pub struct FeeEstimate {
    /// Recommended fee rate in sompi per gram of mass.
    pub normal_bucket_feerate: f64,
    /// Low-priority fee rate (slower inclusion).
    pub low_bucket_feerate: f64,
}

/// Error type for RPC operations.
#[derive(Debug, Clone, Serialize)]
pub enum KaspaError {
    ConnectionFailed(String),
    NodeNotSynced,
    UtxoIndexNotEnabled,
    RpcError(String),
    TransactionFailed(String),
    /// F-003: Specific double-spend error variant for clear error handling.
    DoubleSpend(String),
    /// F-003: Mempool is full, retry later.
    MempoolFull,
    Timeout,
}

impl fmt::Display for KaspaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KaspaError::ConnectionFailed(e) => write!(f, "Connection failed: {}", e),
            KaspaError::NodeNotSynced => write!(f, "Kaspa node is not synced"),
            KaspaError::UtxoIndexNotEnabled => {
                write!(f, "UTXO index not enabled (start node with --utxoindex)")
            }
            KaspaError::RpcError(e) => write!(f, "RPC error: {}", e),
            KaspaError::TransactionFailed(e) => write!(f, "Transaction failed: {}", e),
            KaspaError::DoubleSpend(e) => write!(f, "Double-spend detected: {}", e),
            KaspaError::MempoolFull => write!(f, "Kaspa mempool is full — retry later"),
            KaspaError::Timeout => write!(f, "RPC request timed out"),
        }
    }
}

impl std::error::Error for KaspaError {}

// === KaspaRpc Trait ===

/// Trait abstracting Kaspa RPC operations.
///
/// Implemented by `RealKaspaClient` for production and `MockKaspaClient`
/// for testing. All escrow/payout logic uses `Arc<dyn KaspaRpc>`.
#[async_trait]
pub trait KaspaRpc: Send + Sync {
    /// Establish connection to the Kaspa node.
    async fn connect(&self) -> std::result::Result<(), KaspaError>;

    /// Disconnect from the Kaspa node.
    async fn disconnect(&self) -> std::result::Result<(), KaspaError>;

    /// Check if the client is currently connected.
    async fn is_connected(&self) -> bool;

    /// Check if the connected node is fully synced.
    async fn is_synced(&self) -> std::result::Result<bool, KaspaError>;

    /// Get the balance of an address in sompi.
    async fn get_balance(&self, address: &str) -> std::result::Result<u64, KaspaError>;

    /// Get all UTXOs for an address.
    async fn get_utxos(&self, address: &str) -> std::result::Result<Vec<UtxoInfo>, KaspaError>;

    /// Submit a fully-built and signed Kaspa transaction.
    /// F-003: Takes an RpcTransaction (already converted from consensus Transaction).
    /// Returns the transaction ID on success.
    async fn submit_rpc_transaction(
        &self,
        tx: RpcTransaction,
    ) -> std::result::Result<String, KaspaError>;

    /// Get node status information.
    async fn get_node_info(&self) -> std::result::Result<NodeInfo, KaspaError>;

    /// F-003: Get current fee estimate from the network.
    /// Returns sompi-per-gram fee rates for normal and low-priority transactions.
    async fn get_fee_estimate(&self) -> std::result::Result<FeeEstimate, KaspaError>;

    /// Get the current virtual DAA score from the node.
    /// Used to compute confirmation depth: current_daa - utxo.block_daa_score = confirmations.
    async fn get_current_daa_score(&self) -> std::result::Result<u64, KaspaError>;
}

// === Real Kaspa Client ===

/// Production Kaspa RPC client using kaspa-wrpc-client.
///
/// Connects to a Kaspa node (local or public endpoint) via wRPC
/// with Borsh encoding. Supports reconnection with exponential backoff.
pub struct RealKaspaClient {
    /// The underlying wRPC client
    inner: Arc<kaspa_wrpc_client::KaspaRpcClient>,
    /// Connection URL for logging/reconnection
    node_url: String,
    /// Whether currently connected
    connected: Arc<RwLock<bool>>,
}

impl RealKaspaClient {
    /// Create a new RealKaspaClient and attempt connection.
    ///
    /// Uses wRPC Borsh encoding for optimal performance.
    /// Retries up to 3 times with exponential backoff.
    ///
    /// # Arguments
    /// * `node_url` - wRPC endpoint (e.g. `wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh`)
    /// * `network` - Network identifier (e.g. "testnet-10")
    pub async fn new(node_url: &str, network: &str) -> std::result::Result<Self, KaspaError> {
        use kaspa_wrpc_client::WrpcEncoding;

        tracing::info!("Connecting to Kaspa node: {}", node_url);

        let network_id = Self::parse_network_id(network)?;

        let client = kaspa_wrpc_client::KaspaRpcClient::new(
            WrpcEncoding::Borsh,
            Some(node_url),
            None, // no resolver
            Some(network_id),
            None,
        )
        .map_err(|e| KaspaError::ConnectionFailed(format!("Failed to create client: {}", e)))?;

        let client = Arc::new(client);

        let real_client = Self {
            inner: client,
            node_url: node_url.to_string(),
            connected: Arc::new(RwLock::new(false)),
        };

        // Attempt connection with retry
        real_client.connect_with_retry(3).await?;

        Ok(real_client)
    }

    /// Parse a network string into a NetworkId.
    fn parse_network_id(
        network: &str,
    ) -> std::result::Result<kaspa_consensus_core::network::NetworkId, KaspaError> {
        use kaspa_consensus_core::network::{NetworkId, NetworkType};

        match network {
            "mainnet" => Ok(NetworkId::new(NetworkType::Mainnet)),
            "testnet-10" | "testnet10" => Ok(NetworkId::with_suffix(NetworkType::Testnet, 10)),
            "testnet-11" | "testnet11" => Ok(NetworkId::with_suffix(NetworkType::Testnet, 11)),
            "testnet" => Ok(NetworkId::new(NetworkType::Testnet)),
            _ => Err(KaspaError::ConnectionFailed(format!(
                "Unknown network: {}",
                network
            ))),
        }
    }

    /// Attempt connection with exponential backoff.
    async fn connect_with_retry(&self, max_retries: u32) -> std::result::Result<(), KaspaError> {
        use kaspa_wrpc_client::client::{ConnectOptions, ConnectStrategy};

        let options = ConnectOptions {
            block_async_connect: true,
            connect_timeout: Some(Duration::from_millis(5_000)),
            strategy: ConnectStrategy::Fallback,
            ..Default::default()
        };

        for attempt in 0..max_retries {
            match self.inner.connect(Some(options.clone())).await {
                Ok(_) => {
                    let mut connected = self.connected.write().await;
                    *connected = true;
                    tracing::info!("Connected to Kaspa node: {}", self.node_url);
                    return Ok(());
                }
                Err(e) => {
                    let delay = Duration::from_millis(1000 * 2u64.pow(attempt));
                    tracing::warn!(
                        "Connection attempt {}/{} failed: {} — retrying in {:?}",
                        attempt + 1,
                        max_retries,
                        e,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err(KaspaError::ConnectionFailed(format!(
            "Failed to connect after {} attempts to {}",
            max_retries, self.node_url
        )))
    }

    /// Interpret an RPC submit error message into a specific KaspaError variant.
    /// F-003: Allows callers to distinguish double-spend from mempool-full etc.
    fn classify_submit_error(msg: &str) -> KaspaError {
        let lower = msg.to_lowercase();
        if lower.contains("double") || lower.contains("already spent") {
            KaspaError::DoubleSpend(msg.to_string())
        } else if lower.contains("mempool") && lower.contains("full") {
            KaspaError::MempoolFull
        } else {
            KaspaError::TransactionFailed(msg.to_string())
        }
    }
}

#[async_trait]
impl KaspaRpc for RealKaspaClient {
    async fn connect(&self) -> std::result::Result<(), KaspaError> {
        self.connect_with_retry(3).await
    }

    async fn disconnect(&self) -> std::result::Result<(), KaspaError> {
        self.inner
            .disconnect()
            .await
            .map_err(|e| KaspaError::ConnectionFailed(format!("Disconnect failed: {}", e)))?;
        let mut connected = self.connected.write().await;
        *connected = false;
        tracing::info!("Disconnected from Kaspa node");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        let connected = self.connected.read().await;
        *connected
    }

    async fn is_synced(&self) -> std::result::Result<bool, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;
        let info = self
            .inner
            .get_server_info()
            .await
            .map_err(|e| KaspaError::RpcError(format!("get_server_info failed: {}", e)))?;
        Ok(info.is_synced)
    }

    async fn get_balance(&self, address: &str) -> std::result::Result<u64, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;

        let addr = kaspa_addresses::Address::try_from(address)
            .map_err(|e| KaspaError::RpcError(format!("Invalid address: {}", e)))?;

        let response =
            self.inner.get_balance_by_address(addr).await.map_err(|e| {
                KaspaError::RpcError(format!("get_balance_by_address failed: {}", e))
            })?;

        Ok(response)
    }

    async fn get_utxos(&self, address: &str) -> std::result::Result<Vec<UtxoInfo>, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;

        let addr = kaspa_addresses::Address::try_from(address)
            .map_err(|e| KaspaError::RpcError(format!("Invalid address: {}", e)))?;

        let response = self
            .inner
            .get_utxos_by_addresses(vec![addr])
            .await
            .map_err(|e| KaspaError::RpcError(format!("get_utxos_by_addresses failed: {}", e)))?;

        let utxos = response
            .into_iter()
            .map(|entry| {
                let amount = entry.utxo_entry.amount;
                UtxoInfo {
                    tx_id: entry.outpoint.transaction_id.to_string(),
                    output_index: entry.outpoint.index,
                    amount,
                    amount_kas: amount as f64 / 100_000_000.0, // 1 KAS = 10^8 Sompi
                    is_coinbase: entry.utxo_entry.is_coinbase,
                    block_daa_score: entry.utxo_entry.block_daa_score,
                    script_public_key: Some(hex::encode(
                        entry.utxo_entry.script_public_key.script(),
                    )),
                }
            })
            .collect();

        Ok(utxos)
    }

    /// F-003: Real transaction submission to the Kaspa node.
    ///
    /// Pre-flight check: ensures the node is synced before sending.
    /// Classifies RPC errors into specific KaspaError variants for clear handling.
    async fn submit_rpc_transaction(
        &self,
        tx: RpcTransaction,
    ) -> std::result::Result<String, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;

        // Pre-flight: refuse to submit if node is not synced
        let synced = self.is_synced().await?;
        if !synced {
            return Err(KaspaError::NodeNotSynced);
        }

        let tx_id = self
            .inner
            .submit_transaction(tx, false)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                tracing::error!("submit_transaction failed: {}", msg);
                Self::classify_submit_error(&msg)
            })?;

        let tx_id_str = tx_id.to_string();
        tracing::info!("Transaction submitted successfully: {}", tx_id_str);
        Ok(tx_id_str)
    }

    async fn get_node_info(&self) -> std::result::Result<NodeInfo, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;

        let info = self
            .inner
            .get_server_info()
            .await
            .map_err(|e| KaspaError::RpcError(format!("get_server_info failed: {}", e)))?;

        Ok(NodeInfo {
            server_version: info.server_version.clone(),
            is_synced: info.is_synced,
            is_utxo_indexed: info.has_utxo_index,
            network: info.network_id.to_string(),
        })
    }

    /// F-003: Queries the node for current fee estimates via `get_fee_estimate` RPC.
    ///
    /// Falls back to a reasonable default (1.0 sompi/gram) if the node does not
    /// support this RPC method (older node versions).
    async fn get_fee_estimate(&self) -> std::result::Result<FeeEstimate, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;

        match self.inner.get_fee_estimate().await {
            Ok(response) => {
                // response is RpcFeeEstimate directly with normal_buckets and priority_bucket
                let normal = response
                    .normal_buckets
                    .first()
                    .map(|b| b.feerate)
                    .unwrap_or(response.priority_bucket.feerate);
                let low = response
                    .low_buckets
                    .first()
                    .map(|b| b.feerate)
                    .unwrap_or(0.5);
                Ok(FeeEstimate {
                    normal_bucket_feerate: normal,
                    low_bucket_feerate: low,
                })
            }
            Err(e) => {
                tracing::warn!(
                    "get_fee_estimate not supported by node ({}), using default 1.0 sompi/gram",
                    e
                );
                Ok(FeeEstimate {
                    normal_bucket_feerate: 1.0,
                    low_bucket_feerate: 0.5,
                })
            }
        }
    }

    /// Fetch the current virtual DAA score from get_server_info.
    async fn get_current_daa_score(&self) -> std::result::Result<u64, KaspaError> {
        use kaspa_rpc_core::api::rpc::RpcApi;

        let info = self
            .inner
            .get_server_info()
            .await
            .map_err(|e| KaspaError::RpcError(format!("get_server_info failed: {}", e)))?;

        Ok(info.virtual_daa_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_client_creation_types() {
        let info = NodeInfo {
            server_version: "0.15.0".to_string(),
            is_synced: true,
            is_utxo_indexed: true,
            network: "testnet-10".to_string(),
        };
        assert!(info.is_synced);
        assert!(info.is_utxo_indexed);
    }

    #[test]
    fn test_kaspa_error_display() {
        let err = KaspaError::ConnectionFailed("test".to_string());
        assert_eq!(format!("{}", err), "Connection failed: test");

        let err = KaspaError::NodeNotSynced;
        assert_eq!(format!("{}", err), "Kaspa node is not synced");

        let err = KaspaError::Timeout;
        assert_eq!(format!("{}", err), "RPC request timed out");

        let err = KaspaError::MempoolFull;
        assert!(format!("{}", err).contains("mempool"));

        let err = KaspaError::DoubleSpend("input already spent".to_string());
        assert!(format!("{}", err).contains("Double-spend"));
    }

    #[test]
    fn test_classify_submit_error() {
        let e = RealKaspaClient::classify_submit_error("double spend detected");
        assert!(matches!(e, KaspaError::DoubleSpend(_)));

        let e = RealKaspaClient::classify_submit_error("mempool is full");
        assert!(matches!(e, KaspaError::MempoolFull));

        let e = RealKaspaClient::classify_submit_error("some other error");
        assert!(matches!(e, KaspaError::TransactionFailed(_)));
    }
}
