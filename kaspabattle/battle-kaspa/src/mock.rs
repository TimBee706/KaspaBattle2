/// Mock Kaspa client for testing without a live Kaspa node.
///
/// Implements `KaspaRpc` with in-memory balances, UTXOs, and TX tracking.
/// Used automatically when no Kaspa node is reachable at startup.
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::rpc::{KaspaError, KaspaRpc, NodeInfo, UtxoInfo};

/// A mock implementation of KaspaRpc for development and testing.
///
/// Stores balances, UTXOs, and submitted transactions in memory.
/// All operations are deterministic and instant.
pub struct MockKaspaClient {
    balances: Arc<Mutex<HashMap<String, u64>>>,
    utxos: Arc<Mutex<HashMap<String, Vec<UtxoInfo>>>>,
    submitted_txs: Arc<Mutex<Vec<String>>>,
    connected: Arc<Mutex<bool>>,
}

impl MockKaspaClient {
    /// Create a new mock client in connected state.
    pub fn new() -> Self {
        MockKaspaClient {
            balances: Arc::new(Mutex::new(HashMap::new())),
            utxos: Arc::new(Mutex::new(HashMap::new())),
            submitted_txs: Arc::new(Mutex::new(Vec::new())),
            connected: Arc::new(Mutex::new(true)),
        }
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

    /// Get all submitted transactions (for test assertions).
    pub fn get_submitted_txs(&self) -> Vec<String> {
        let txs = self.submitted_txs.lock().expect("lock poisoned");
        txs.clone()
    }

    /// Set the connected state.
    pub fn set_connected(&self, connected: bool) {
        let mut c = self.connected.lock().expect("lock poisoned");
        *c = connected;
    }
}

impl Default for MockKaspaClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KaspaRpc for MockKaspaClient {
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
        Ok(true)
    }

    async fn get_balance(&self, address: &str) -> Result<u64, KaspaError> {
        let balances = self.balances.lock().expect("lock poisoned");
        Ok(*balances.get(address).unwrap_or(&0))
    }

    async fn get_utxos(&self, address: &str) -> Result<Vec<UtxoInfo>, KaspaError> {
        let utxos = self.utxos.lock().expect("lock poisoned");
        Ok(utxos.get(address).cloned().unwrap_or_default())
    }

    async fn submit_transaction(&self, tx_hex: &str) -> Result<String, KaspaError> {
        let mut txs = self.submitted_txs.lock().expect("lock poisoned");
        txs.push(tx_hex.to_string());

        // Generate a deterministic mock TX ID from the input
        let mut hasher = Sha256::new();
        hasher.update(tx_hex.as_bytes());
        let hash = hasher.finalize();
        let tx_id = hex::encode(&hash[..32]);
        Ok(tx_id)
    }

    async fn get_node_info(&self) -> Result<NodeInfo, KaspaError> {
        let connected = self.connected.lock().expect("lock poisoned");
        if !*connected {
            return Err(KaspaError::ConnectionFailed(
                "Mock: not connected".to_string(),
            ));
        }
        Ok(NodeInfo {
            server_version: "mock-0.15.0".to_string(),
            is_synced: true,
            is_utxo_indexed: true,
            network: "testnet-10".to_string(),
        })
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
    async fn test_mock_client_submit_tx() {
        let client = MockKaspaClient::new();
        let tx_id = client.submit_transaction("raw_tx_data").await.unwrap();
        assert!(!tx_id.is_empty());
        assert_eq!(tx_id.len(), 64); // SHA-256 hex

        let txs = client.get_submitted_txs();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0], "raw_tx_data");
    }

    #[tokio::test]
    async fn test_mock_client_node_info() {
        let client = MockKaspaClient::new();
        let info = client.get_node_info().await.unwrap();
        assert!(info.is_synced);
        assert!(info.is_utxo_indexed);
        assert_eq!(info.server_version, "mock-0.15.0");
        assert_eq!(info.network, "testnet-10");
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
    async fn test_mock_client_disconnected_node_info() {
        let client = MockKaspaClient::new();
        client.set_connected(false);
        let result = client.get_node_info().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_fallback_concept() {
        // Verify MockKaspaClient can be used as Arc<dyn KaspaRpc>
        let client: Arc<dyn KaspaRpc> = Arc::new(MockKaspaClient::new());
        // This compiles = trait object works
        let _ = client;
    }
}
