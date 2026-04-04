/// EscrowWallet — BIP44-based HD wallet for Kaspa escrow address derivation.
///
/// Each challenge gets a unique derivation index, ensuring funds are isolated
/// per-escrow. Private keys are held in memory behind Arc<Mutex<>> for signing
/// payouts later.
///
/// F-003: The mnemonic phrase is wrapped in `Zeroizing<String>` so that it is
///        automatically wiped from heap memory when the wallet is dropped.
/// F-008: `Debug` is manually implemented to redact the mnemonic.
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_bip32::{
    secp256k1::SecretKey, DerivationPath, ExtendedPrivateKey, Language, Mnemonic, WordCount,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Kaspa BIP44 coin type (111111, NOT 111)
const KASPA_COIN_TYPE: u32 = 111111;

/// Stores the master key + derived key cache for escrow operations.
///
/// **Security**: `Debug` is manually implemented to never expose the mnemonic.
pub struct EscrowWallet {
    /// F-003: The mnemonic phrase (12 words), zeroized on drop.
    #[allow(dead_code)]
    mnemonic_phrase: Zeroizing<String>,
    /// Master extended private key
    master_xprv: ExtendedPrivateKey<SecretKey>,
    /// Network prefix for address generation
    prefix: Prefix,
    /// Map: address_string → 32-byte private key
    keys: Arc<Mutex<HashMap<String, [u8; 32]>>>,
    /// Map: challenge_id → derivation index (deterministic)
    index_map: Arc<Mutex<HashMap<String, u32>>>,
}

/// F-008: Safe Debug implementation that redacts the mnemonic phrase.
impl fmt::Debug for EscrowWallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EscrowWallet")
            .field("mnemonic_phrase", &"[REDACTED]")
            .field("prefix", &self.prefix)
            .finish()
    }
}

impl EscrowWallet {
    /// Creates a new EscrowWallet.
    ///
    /// - If `mnemonic` is `Some`, imports existing 12-word mnemonic.
    /// - If `mnemonic` is `None`, generates a fresh 12-word mnemonic.
    ///
    /// # Arguments
    /// * `mnemonic` - Optional existing mnemonic phrase
    /// * `network` - "testnet" or "mainnet"
    pub fn new(mnemonic: Option<String>, network: &str) -> Result<Self> {
        let (phrase, m) = match mnemonic {
            Some(ref words) => {
                let m = Mnemonic::new(words, Language::English)
                    .map_err(|e| anyhow!("Invalid mnemonic: {}", e))?;
                (Zeroizing::new(words.clone()), m)
            }
            None => {
                let m = Mnemonic::random(WordCount::Words12, Language::English)
                    .map_err(|e| anyhow!("Failed to generate mnemonic: {}", e))?;
                let phrase = Zeroizing::new(m.phrase().to_string());
                (phrase, m)
            }
        };

        let seed = m.to_seed("");
        let master_xprv = ExtendedPrivateKey::<SecretKey>::new(seed)
            .map_err(|e| anyhow!("Failed to create master key: {}", e))?;

        let prefix = if network == "mainnet" {
            Prefix::Mainnet
        } else {
            Prefix::Testnet
        };

        // F-008: Use log:: (not tracing::) for consistency. Never log secrets.
        log::info!(
            "EscrowWallet initialized (network: {}, mnemonic: {})",
            network,
            if mnemonic.is_some() {
                "imported"
            } else {
                "generated"
            }
        );

        Ok(Self {
            mnemonic_phrase: phrase,
            master_xprv,
            prefix,
            keys: Arc::new(Mutex::new(HashMap::new())),
            index_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Derives a deterministic escrow address for a challenge.
    ///
    /// The derivation index is determined by hashing the challenge_id.
    /// Same challenge_id always produces the same address.
    ///
    /// Returns (Address, derivation_index).
    pub fn derive_escrow_address(&self, challenge_id: &str) -> Result<(Address, u32)> {
        if challenge_id.is_empty() {
            return Err(anyhow!("Challenge ID cannot be empty"));
        }

        // Check if we already derived this one
        {
            let index_map = self
                .index_map
                .lock()
                .map_err(|_| anyhow!("Lock poisoned"))?;
            if let Some(&idx) = index_map.get(challenge_id) {
                // If we have the index, we need to reconstruct the address.
                // The address is not stored directly in index_map or keys in a way
                // that allows direct lookup by index.
                // So, we re-derive the address using the stored index.
                // This ensures consistency and avoids issues if `keys` map was cleared.
                let path = format!("m/44'/{}'/{}'/{}/{}", KASPA_COIN_TYPE, 0, 0, idx);
                let derivation_path = path
                    .parse::<DerivationPath>()
                    .map_err(|e| anyhow!("Invalid derivation path: {}", e))?;

                let child_xprv = self
                    .master_xprv
                    .clone()
                    .derive_path(&derivation_path)
                    .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

                let secret_bytes = child_xprv.private_key().secret_bytes();

                let secp = secp256k1::Secp256k1::new();
                let sk = secp256k1::SecretKey::from_slice(&secret_bytes)
                    .map_err(|e| anyhow!("Invalid secret key: {}", e))?;
                let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
                let (xonly_pubkey, _parity) = keypair.x_only_public_key();
                let pubkey_bytes = xonly_pubkey.serialize();

                let address = Address::new(self.prefix, Version::PubKey, &pubkey_bytes);

                // Ensure the private key is still in the `keys` map.
                // If not, add it back. This handles cases where `keys` might be
                // cleared or not fully consistent with `index_map`.
                let addr_string = address.to_string();
                let mut keys = self.keys.lock().map_err(|_| anyhow!("Lock poisoned"))?;
                keys.entry(addr_string.clone()).or_insert(secret_bytes);

                return Ok((address, idx));
            }
        }

        // Deterministic index from challenge_id hash
        let index = self.challenge_to_index(challenge_id);

        // Derive: m/44'/111111'/0'/0/index
        let path = format!("m/44'/{}'/{}'/{}/{}", KASPA_COIN_TYPE, 0, 0, index);
        let derivation_path = path
            .parse::<DerivationPath>()
            .map_err(|e| anyhow!("Invalid derivation path: {}", e))?;

        let child_xprv = self
            .master_xprv
            .clone()
            .derive_path(&derivation_path)
            .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

        let secret_bytes = child_xprv.private_key().secret_bytes();

        // Generate Schnorr public key (x-only) from private key
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&secret_bytes)
            .map_err(|e| anyhow!("Invalid secret key: {}", e))?;
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let (xonly_pubkey, _parity) = keypair.x_only_public_key();
        let pubkey_bytes = xonly_pubkey.serialize();

        // Create Kaspa address (Schnorr / P2PK)
        let address = Address::new(self.prefix, Version::PubKey, &pubkey_bytes);

        // Store private key and index mapping
        let addr_string = address.to_string();
        {
            let mut keys = self.keys.lock().map_err(|_| anyhow!("Lock poisoned"))?;
            keys.insert(addr_string, secret_bytes);
        }
        {
            let mut index_map = self
                .index_map
                .lock()
                .map_err(|_| anyhow!("Lock poisoned"))?;
            index_map.insert(challenge_id.to_string(), index);
        }

        Ok((address, index))
    }

    /// Retrieves the 32-byte private key for a previously derived address.
    pub fn get_private_key(&self, address: &Address) -> Result<[u8; 32]> {
        let addr_str = address.to_string();
        let keys = self.keys.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        keys.get(&addr_str)
            .copied()
            .ok_or_else(|| anyhow!("Private key not found for address: {}", addr_str))
    }

    /// Convenience: derives and returns just the address string for a challenge.
    pub fn get_escrow_address_for_challenge(&self, challenge_id: &str) -> Result<Address> {
        let (addr, _) = self.derive_escrow_address(challenge_id)?;
        Ok(addr)
    }

    /// Maps a challenge_id to a deterministic derivation index via SHA-256.
    fn challenge_to_index(&self, challenge_id: &str) -> u32 {
        let mut hasher = Sha256::new();
        hasher.update(challenge_id.as_bytes());
        let hash = hasher.finalize();
        // Take first 4 bytes as u32, mask to reasonable range (0..2^20)
        let raw = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        raw % (1 << 20) // Max ~1M unique indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn test_wallet_from_mnemonic() {
        let wallet = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet");
        assert!(wallet.is_ok());
        // F-003: verify derivation works, do not assert mnemonic equality
        let w = wallet.unwrap();
        let (addr, _) = w.derive_escrow_address("test-init").unwrap();
        assert!(!addr.to_string().is_empty());
    }

    #[test]
    fn test_wallet_generate_new() {
        let wallet = EscrowWallet::new(None, "testnet").unwrap();
        // F-003: verify 12-word mnemonic was generated without exposing it
        let phrase = wallet.mnemonic_phrase.as_str();
        let word_count = phrase.split_whitespace().count();
        assert_eq!(
            word_count, 12,
            "Mnemonic should be 12 words, got {}",
            word_count
        );
    }

    #[test]
    fn test_debug_does_not_leak_mnemonic() {
        // F-008: ensure Debug output redacts mnemonic
        let wallet = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();
        let debug_output = format!("{:?}", wallet);
        assert!(
            !debug_output.contains("abandon"),
            "Debug output must not contain mnemonic words"
        );
        assert!(
            debug_output.contains("REDACTED"),
            "Debug output must show REDACTED for mnemonic"
        );
    }

    #[test]
    fn test_derive_escrow_address() {
        let wallet = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();

        let mut addresses = Vec::new();
        for i in 0..10 {
            let challenge_id = format!("challenge-{}", i);
            let (addr, _idx) = wallet.derive_escrow_address(&challenge_id).unwrap();
            let addr_str = addr.to_string();
            assert!(
                !addresses.contains(&addr_str),
                "Duplicate address at index {}",
                i
            );
            addresses.push(addr_str);
        }
        assert_eq!(addresses.len(), 10);
    }

    #[test]
    fn test_derive_deterministic() {
        let wallet1 = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();
        let wallet2 = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();

        let (addr1, idx1) = wallet1.derive_escrow_address("test-challenge").unwrap();
        let (addr2, idx2) = wallet2.derive_escrow_address("test-challenge").unwrap();

        assert_eq!(addr1.to_string(), addr2.to_string());
        assert_eq!(idx1, idx2);
    }

    #[test]
    fn test_address_format() {
        let wallet = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();
        let (addr, _) = wallet
            .derive_escrow_address("challenge-format-test")
            .unwrap();
        let addr_str = addr.to_string();
        assert!(
            addr_str.starts_with("kaspatest:"),
            "Testnet address should start with kaspatest:, got {}",
            addr_str
        );

        // Mainnet test
        let wallet_main = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "mainnet").unwrap();
        let (addr_main, _) = wallet_main
            .derive_escrow_address("challenge-format-test")
            .unwrap();
        let addr_main_str = addr_main.to_string();
        assert!(
            addr_main_str.starts_with("kaspa:"),
            "Mainnet address should start with kaspa:, got {}",
            addr_main_str
        );
    }

    #[test]
    fn test_private_key_retrieval() {
        let wallet = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();
        let (addr, _) = wallet.derive_escrow_address("challenge-key-test").unwrap();

        let privkey = wallet.get_private_key(&addr);
        assert!(privkey.is_ok());
        let key_bytes = privkey.unwrap();
        assert_eq!(key_bytes.len(), 32);
        // Must not be all zeros
        assert!(key_bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_sign_verify() {
        let wallet = EscrowWallet::new(Some(TEST_MNEMONIC.to_string()), "testnet").unwrap();
        let (addr, _) = wallet.derive_escrow_address("challenge-sign-test").unwrap();
        let privkey_bytes = wallet.get_private_key(&addr).unwrap();

        // Sign a message with Schnorr
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&privkey_bytes).unwrap();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let message = secp256k1::Message::from_digest([42u8; 32]);
        let sig = secp.sign_schnorr(&message, &keypair);

        // Verify
        let (xonly, _) = keypair.x_only_public_key();
        let verify_result = secp.verify_schnorr(&sig, &message, &xonly);
        assert!(
            verify_result.is_ok(),
            "Schnorr signature verification failed"
        );
    }
}
