use async_trait::async_trait;
use kaspa_rpc_core::model::tx::RpcTransaction;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::rpc::{FeeEstimate, KaspaBackend, KaspaError, NodeInfo, TxInputInfo, UtxoInfo};

/// A mock implementation of KaspaRpc for development and testing.
///
/// Stores balances, UTXOs, and submitted transactions in memory.
/// All operations are deterministic and instant.
pub struct MockKaspaClient {
    balances: Arc<Mutex<HashMap<String, u64>>>,
    utxos: Arc<Mutex<HashMap<String, Vec<UtxoInfo>>>>,
    submitted_tx_ids: Arc<Mutex<Vec<String>>>,
    connected: Arc<Mutex<bool>>,
    /// Controls `is_synced()` response for testing node-not-ready scenarios.
    synced: Arc<Mutex<bool>>,
    /// Controls `get_current_daa_score()` response (default: 1000).
    current_daa_score: Arc<Mutex<u64>>,
    /// Registered fake transactions for `get_transaction()` responses.
    /// Key: tx_id string, Value: list of TxInputInfo.
    registered_transactions: Arc<Mutex<HashMap<String, Vec<TxInputInfo>>>>,
}

impl MockKaspaClient {
    /// Create a new mock client in connected + synced state.
    pub fn new() -> Self {
        MockKaspaClient {
            balances: Arc::new(Mutex::new(HashMap::new())),
            utxos: Arc::new(Mutex::new(HashMap::new())),
            submitted_tx_ids: Arc::new(Mutex::new(Vec::new())),
            connected: Arc::new(Mutex::new(true)),
            synced: Arc::new(Mutex::new(true)),
            current_daa_score: Arc::new(Mutex::new(1000)),
            registered_transactions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a mock client with a specific sync state (for testing F-003 node-not-ready).
    pub fn with_synced(synced: bool) -> Self {
        let client = Self::new();
        *client.synced.lock().expect("lock") = synced;
        client
    }

    /// Set the current DAA score (for testing confirmation logic).
    pub fn set_daa_score(&self, score: u64) {
        let mut s = self.current_daa_score.lock().expect("lock poisoned");
        *s = score;
    }


    /// Set the balance for an address (in sompi).
    pub fn set_balance(&self, address: &str, balance: u64) {
        let mut balances = self.balances.lock().expect("lock poisoned");
        balances.insert(address.to_string(), balance);
    }

    /// Add a UTXO to an address.
    pub fn add_utxo(&self, address: &str, utxo: UtxoInfo) {
        let mut utxos = self.utxos.lock().expect("lock poisoned");
        utxos.entry(address.to_string()).or_default().push(utxo);
    }

    /// Get all submitted transaction IDs (for test assertions).
    pub fn get_submitted_tx_ids(&self) -> Vec<String> {
        let txs = self.submitted_tx_ids.lock().expect("lock poisoned");
        txs.clone()
    }

    /// Set the connected state.
    pub fn set_connected(&self, connected: bool) {
        let mut c = self.connected.lock().expect("lock poisoned");
        *c = connected;
    }

    /// Register a fake transaction for `get_transaction()` test responses.
    ///
    /// # Example
    /// ```
    /// mock.register_transaction(
    ///     "abc123",
    ///     vec![TxInputInfo {
    ///         previous_tx_id: "prev_tx".into(),
    ///         previous_output_index: 0,
    ///         sender_address: Some("kaspatest:qalice".into()),
    ///         amount_sompi: 10_000_000_000,
    ///     }],
    /// );
    /// ```
    pub fn register_transaction(&self, tx_id: &str, inputs: Vec<TxInputInfo>) {
        let mut txs = self.registered_transactions.lock().expect("lock poisoned");
        txs.insert(tx_id.to_string(), inputs);
    }
}

impl Default for MockKaspaClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KaspaBackend for MockKaspaClient {
    async fn connect(&self) -> Result<(), KaspaError> {
        let mut c = self.connected.lock().expect("lock poisoned");
        *c = true;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), KaspaError> {
        let mut c = self.connected.lock().expect("lock poisoned");
        *c = false;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        let connected = self.connected.lock().expect("lock poisoned");
        *connected
    }

    async fn is_synced(&self) -> Result<bool, KaspaError> {
        let connected = self.connected.lock().expect("lock poisoned");
        if !*connected {
            return Err(KaspaError::ConnectionFailed(
                "Mock: not connected".to_string(),
            ));
        }
        let synced = self.synced.lock().expect("lock poisoned");
        Ok(*synced)
    }

    async fn get_balance(&self, address: &str) -> Result<u64, KaspaError> {
        let balances = self.balances.lock().expect("lock poisoned");
        Ok(*balances.get(address).unwrap_or(&0))
    }

    async fn get_utxos(&self, address: &str) -> Result<Vec<UtxoInfo>, KaspaError> {
        let utxos = self.utxos.lock().expect("lock poisoned");
        Ok(utxos.get(address).cloned().unwrap_or_default())
    }

    /// Accept a serialized TX payload, derive a mock TX ID, and record it.
    async fn submit_transaction(
        &self,
        tx_payload: &str,
    ) -> Result<String, KaspaError> {
        let synced = *self.synced.lock().expect("lock poisoned");
        if !synced {
            return Err(KaspaError::NodeNotSynced);
        }

        let tx: RpcTransaction = serde_json::from_str(tx_payload)
            .map_err(|e| KaspaError::TransactionFailed(format!("Failed to parse mock tx json: {}", e)))?;

        // Deterministic mock TX ID from inputs
        let mut hasher = Sha256::new();
        for inp in &tx.inputs {
            hasher.update(inp.previous_outpoint.transaction_id.as_bytes());
            hasher.update(inp.previous_outpoint.index.to_le_bytes());
        }
        let tx_id = hex::encode(hasher.finalize());
        let mut txs = self.submitted_tx_ids.lock().expect("lock poisoned");
        txs.push(tx_id.clone());
        Ok(tx_id)
    }

    async fn get_node_info(&self) -> Result<NodeInfo, KaspaError> {
        let connected = self.connected.lock().expect("lock poisoned");
        if !*connected {
            return Err(KaspaError::ConnectionFailed(
                "Mock: not connected".to_string(),
            ));
        }
        let synced = *self.synced.lock().expect("lock poisoned");
        Ok(NodeInfo {
            server_version: "mock-0.15.0".to_string(),
            is_synced: synced,
            is_utxo_indexed: true,
            network: "testnet-12".to_string(),
        })
    }

    async fn get_fee_estimate(&self) -> Result<FeeEstimate, KaspaError> {
        Ok(FeeEstimate {
            normal_bucket_feerate: 1.0,
            low_bucket_feerate: 0.5,
        })
    }

    async fn get_current_daa_score(&self) -> Result<u64, KaspaError> {
        let score = self.current_daa_score.lock().expect("lock poisoned");
        Ok(*score)
    }

    async fn wait_for_sync(&self, _timeout: std::time::Duration) -> Result<(), KaspaError> {
        // Mock is always ready
        Ok(())
    }

    async fn get_transaction(
        &self,
        tx_id: &str,
    ) -> Result<Option<Vec<TxInputInfo>>, KaspaError> {
        let txs = self.registered_transactions.lock().expect("lock poisoned");
        Ok(txs.get(tx_id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_client_balance() {
        let client = MockKaspaClient::new();
        client.set_balance("kaspatest:qabc", 5_000_000);

        let balance = client.get_balance("kaspatest:qabc").await.unwrap();
        assert_eq!(balance, 5_000_000);

        let zero = client.get_balance("kaspatest:qunknown").await.unwrap();
        assert_eq!(zero, 0);
    }

    #[tokio::test]
    async fn test_mock_client_utxos() {
        let client = MockKaspaClient::new();
        client.add_utxo(
            "kaspatest:qabc",
            UtxoInfo {
                tx_id: "tx001".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 50.0,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: None,
            },
        );

        let utxos = client.get_utxos("kaspatest:qabc").await.unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].amount, 5_000_000);
    }

    #[tokio::test]
    async fn test_mock_client_node_info() {
        let client = MockKaspaClient::new();
        let info = client.get_node_info().await.unwrap();
        assert!(info.is_synced);
        assert!(info.is_utxo_indexed);
        assert_eq!(info.server_version, "mock-0.15.0");
        assert_eq!(info.network, "testnet-12");
    }

    #[tokio::test]
    async fn test_mock_client_connect_disconnect() {
        let client = MockKaspaClient::new();
        assert!(client.is_connected().await);

        client.disconnect().await.unwrap();
        assert!(!client.is_connected().await);

        client.connect().await.unwrap();
        assert!(client.is_connected().await);
    }

    #[tokio::test]
    async fn test_mock_client_not_synced() {
        let client = MockKaspaClient::with_synced(false);
        let synced = client.is_synced().await.unwrap();
        assert!(!synced);
    }

    #[tokio::test]
    async fn test_mock_fee_estimate() {
        let client = MockKaspaClient::new();
        let estimate = client.get_fee_estimate().await.unwrap();
        assert!(estimate.normal_bucket_feerate > 0.0);
    }

    #[test]
    fn test_mock_fallback_concept() {
        let client: Arc<dyn KaspaBackend> = Arc::new(MockKaspaClient::new());
        let _ = client;
    }
}
