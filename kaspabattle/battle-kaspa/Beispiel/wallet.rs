// ===== IMPORTS =====
use anyhow::{Result, anyhow};
use kaspa_wallet_core::prelude::*;
use kaspa_wallet_core::storage::keydata::data::PrvKeyDataVariantKind;
use kaspa_wallet_core::wallet::WalletOpenArgs;
use workflow_core::abortable::Abortable;

use crate::models::WalletInfo;
use kaspa_wallet_core::prelude::Address;

use kaspa_wallet_core::tx::{PaymentDestination, PaymentOutput, Fees, PendingTransaction};
use std::sync::Arc;
use std::convert::TryFrom;

use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding, Resolver};
use kaspa_consensus_core::network::NetworkId;
use tokio::sync::Mutex;
use tracing::info;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::models::{KaspaWallet, TimelockResponse};

impl KaspaWallet {
    /// Konstruktor mit expliziter RPC-URL (z.B. für lokale Node).
    pub async fn new_with_url(network_id: NetworkId, rpc_url: &str) -> Result<Self> {
        let rpc_client = Arc::new(
            KaspaRpcClient::new(
                WrpcEncoding::Borsh,
                Some(rpc_url),
                None,
                Some(network_id),
                None,
            )?
        );

        timeout(Duration::from_secs(10), rpc_client.connect(None)).await
            .map_err(|_| anyhow!("Connection timeout after 10 seconds. Is the node running at {}?", rpc_url))??;

        let store = Wallet::local_store()?;
        let wallet = Wallet::try_new(store, None, Some(network_id))?;
        let wallet = Arc::new(wallet);

        wallet.set_network_id(&network_id)?;
        wallet.wrpc_client().set_url(Some(rpc_url))?;

        let wallet_secret = Secret::from("change-me-wallet-secret".to_string());
        let wallet_name = "kaspa-scheduler-wallet";

        if wallet.descriptor().is_none() {
            info!("📁 Creating new wallet: {}", wallet_name);
            let wallet_args = WalletCreateArgs::new(
                Some(wallet_name.to_string()),
                None,
                EncryptionKind::default(),
                None,
                true,
            );
            wallet.create_wallet(&wallet_secret, wallet_args).await?;
        } else {
            info!("📁 Opening existing wallet: {}", wallet_name);
            let open_args = WalletOpenArgs {
                account_descriptors: true,
                legacy_accounts: false,
            };

            let guard_mutex = wallet.guard();
            let guard = guard_mutex.lock().await;

            wallet
                .open(&wallet_secret, Some(wallet_name.to_string()), open_args, &guard)
                .await?;
        }

        wallet.start().await?;

        Ok(Self {
            wallet,
            account_id: Arc::new(Mutex::new(None)),
            rpc_client,
            network_id,
            mnemonic: Arc::new(Mutex::new(None)),
        })
    }
}

/// Public function: Create a new wallet asynchronously
pub async fn create_wallet_async() -> Result<String> {
    let wallet_id = Uuid::new_v4().to_string();
    info!("Created new wallet with ID: {}", wallet_id);
    Ok(wallet_id)
}

impl KaspaWallet {
    /// Standard-Konstruktor: nutzt Resolver statt fester URL.
    pub async fn new(network_id: NetworkId) -> Result<Self> {
        // wRPC-Client mit Resolver statt fixer URL
        let rpc_client = Arc::new(
            KaspaRpcClient::new(
                WrpcEncoding::Borsh,
                None,                      // keine feste URL
                Some(Resolver::default()), // Resolver wählt Node
                Some(network_id),
                None,
            )?
        );

        // Optional: Timeout wie bisher
        timeout(Duration::from_secs(10), rpc_client.connect(None)).await
            .map_err(|_| anyhow!("Connection timeout after 10 seconds using resolver"))??;

        let store = Wallet::local_store()?;
        let wallet = Wallet::try_new(store, None, Some(network_id))?;
        let wallet = Arc::new(wallet);

        wallet.set_network_id(&network_id)?;

        // Hier KEINE feste URL setzen, der Client nutzt den Resolver:
        // wallet.wrpc_client().set_url(...) ist nicht nötig

        let wallet_secret = Secret::from("change-me-wallet-secret".to_string());
        let wallet_name = "kaspa-scheduler-wallet";

        if wallet.descriptor().is_none() {
            info!("📁 Creating new wallet: {}", wallet_name);
            let wallet_args = WalletCreateArgs::new(
                Some(wallet_name.to_string()),
                None,
                EncryptionKind::default(),
                None,
                true,
            );
            wallet.create_wallet(&wallet_secret, wallet_args).await?;
        } else {
            info!("📁 Opening existing wallet: {}", wallet_name);
            let open_args = WalletOpenArgs {
                account_descriptors: true,
                legacy_accounts: false,
            };

            let guard_mutex = wallet.guard();
            let guard = guard_mutex.lock().await;

            wallet
                .open(&wallet_secret, Some(wallet_name.to_string()), open_args, &guard)
                .await?;
        }

        wallet.start().await?;

        Ok(Self {
            wallet,
            account_id: Arc::new(Mutex::new(None)),
            rpc_client,
            network_id,
            mnemonic: Arc::new(Mutex::new(None)),
        })
    }


    
    pub async fn create_account(&self, mnemonic_input: Option<String>) -> Result<String> {
        let mut mnemonic_guard = self.mnemonic.lock().await;

        // 1) Mnemonic-Secret vorbereiten
        let mnemonic_secret = if let Some(phrase) = mnemonic_input {
            info!("📥 Importing mnemonic");
            Secret::from(phrase)
        } else {
            let mnemonic = Mnemonic::random(WordCount::Words24, Language::English)?;
            let phrase = mnemonic.phrase_string();
            info!("═══════════════════════════════════════");
            info!("🔑 KASPA MNEMONIC (SAVE THIS!):");
            info!("   {}", phrase);
            info!("═══════════════════════════════════════");
            Secret::from(phrase)
        };

        *mnemonic_guard = Some(String::from_utf8(mnemonic_secret.as_ref().to_vec())?);

        // 2) Secrets
        let wallet_secret = Secret::from("change-me-wallet-secret".to_string());
        let payment_secret: Option<Secret> = None;

        // 3) Guard
        let guard_mutex = self.wallet.guard();
        let guard = guard_mutex.lock().await;

        // 4) PrvKeyData + Account erstellen (Wallet ist jetzt immer offen)
        let prv_key_data_args = PrvKeyDataCreateArgs::new(
            None,
            payment_secret.clone(),
            mnemonic_secret,
            PrvKeyDataVariantKind::Mnemonic,
        );

        self.wallet.store().batch().await?;
        let prv_key_data_id = self
            .wallet
            .create_prv_key_data(&wallet_secret, prv_key_data_args)
            .await?;

        let account_create_args = AccountCreateArgs::new_bip32(prv_key_data_id, payment_secret.clone(), None, None);

        let account = self
            .wallet
            .create_account(&wallet_secret, account_create_args, true, &guard)
            .await?;

        // 5) Account-ID merken
        {
            let mut id_guard = self.account_id.lock().await;
            *id_guard = Some(*account.id());
        }

        info!("✅ Account created and stored in wallet");

        Ok("Account created and stored".to_string())
    }


    pub async fn get_balance(&self) -> Result<u64> 
    {
            // 1) Account-ID lesen
            let account_id = {
                let id_guard = self.account_id.lock().await;
                *id_guard
            }
            .ok_or_else(|| anyhow!("No account. Call create_account first."))?;

            // 2) Wallet-Guard holen
            let guard_mutex = self.wallet.guard();
            let wallet_guard = guard_mutex.lock().await;

            // 3) Account (falls nötig) aktivieren
            self.wallet
                .activate_accounts(Some(&[account_id]), &wallet_guard)
                .await?;

            // 4) Account-Instanz holen
            let account = self
                .wallet
                .get_account_by_id(&account_id, &wallet_guard)
                .await?
                .ok_or_else(|| anyhow!("Account not found"))?;

            // 5) Balance lesen; wenn noch nicht verfügbar, 0 zurückgeben
            let balance = account.balance();
            let mature = balance.map(|b| b.mature).unwrap_or(0);

            Ok(mature)
        }


    pub async fn create_timelock_tx(
        &self,
        destination: String,
        amount: u64,
        _locktime: u64,
    ) -> Result<TimelockResponse> {
        // 1) Account-ID sicherstellen
        let account_id = {
            let id_guard = self.account_id.lock().await;
            *id_guard
        }
        .ok_or_else(|| anyhow!("No account. Call create_account first."))?;

        // 2) Wallet-Guard + Account holen
        let guard_mutex = self.wallet.guard();
        let wallet_guard = guard_mutex.lock().await;

        // Account aktivieren (falls nötig)
        self.wallet
            .activate_accounts(Some(&[account_id]), &wallet_guard)
            .await?;

        let account = self
            .wallet
            .get_account_by_id(&account_id, &wallet_guard)
            .await?
            .ok_or_else(|| anyhow!("Account not found"))?;

        // 3) Zieladresse parsen (kaspa_addresses)
        let address = Address::try_from(destination.as_str())
            .map_err(|e| anyhow!("Invalid address `{destination}`: {e}"))?;


        // 4) PaymentDestination aufbauen
        let output = PaymentOutput::new(address, amount);
        let destination = PaymentDestination::from(output);  // statt .Outputs / Struct-Literal

        // 5) Standard-Gebühren
        let fee_rate: Option<f64> = None;
        let priority_fee = Fees::None;     // oder passend aus deiner Fee-Policy
        let payload: Option<Vec<u8>> = None;


        // 6) Wallet-/Payment-Secret für Signatur
        let wallet_secret = Secret::from("change-me-wallet-secret".to_string());
        let payment_secret: Option<Secret> = None;

        // 7) Abortable + progress-Callback
        let abortable = Abortable::default();
        let progress: Option<Arc<dyn Fn(&PendingTransaction) + Send + Sync>> = None;

        // 8) Senden: Account::send kapselt Generator + Signer + submit_transaction
        let (_summary, tx_ids) = account
            .clone()
            .send(
                destination,
                fee_rate,
                priority_fee,
                payload,
                wallet_secret,
                payment_secret,
                &abortable,
                progress,
            )
            .await
            .map_err(|e| anyhow!("Failed to send transaction: {e}"))?;

        // Es können mehrere TXs entstehen (Batch). Wir nehmen die erste.
        let tx_id = tx_ids
            .get(0)
            .ok_or_else(|| anyhow!("No transaction id returned from send()"))?
            .to_string();

        // 9) raw_tx aktuell nicht verfügbar -> leer lassen
        let raw_tx = String::new();

        Ok(TimelockResponse {
            tx_id,
            locktime: 0, // aktuell ungenutzt
            raw_tx,
            status: "sent".to_string(),
        })
    }


    /// Gibt den gespeicherten Mnemonic zurück (für Export/Backup)
    pub async fn get_mnemonic(&self) -> Result<String> {
        let mnemonic_guard = self.mnemonic.lock().await;
        mnemonic_guard
            .clone()
            .ok_or_else(|| anyhow!("No mnemonic available. Create account first."))
    }
}

impl KaspaWallet {
    pub async fn to_wallet_info(&self) -> WalletInfo {
        // mnemonic aus dem Mutex holen (Arc<Mutex<Option<String>>>)
        let mnemonic = self.mnemonic.lock().await.clone();

        // account_id in String wandeln
        let id = {
            let id_guard = self.account_id.lock().await;
            id_guard
                .map(|id| id.to_string())
                .unwrap_or_else(|| "no-account".to_string())
        };

        WalletInfo {
            id,
            name: None,
            mnemonic,
        }
    }
}

