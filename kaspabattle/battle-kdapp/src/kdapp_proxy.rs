//! Proxy — Kaspa wRPC client setup and TX listener.
//!
//! Inlined from michaelsutton/kdapp (kdapp/src/proxy.rs).
//! Adapted to use kaspa v0.15 types and local module paths.
//!
//! Provides:
//! - `connect_client()` — Resolver-based node connection
//! - `run_listener()` — Polls the virtual chain for pattern-matching TXs

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use kaspa_consensus_core::network::NetworkId;
use kaspa_hashes::Hash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::prelude::*;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use log::{debug, info, warn};
use tokio::time::{sleep_until, Instant};

use crate::kdapp_engine::EngineMsg as Msg;
use crate::kdapp_generator::{check_pattern, PatternType, Payload, PrefixType};

fn connect_options() -> ConnectOptions {
    ConnectOptions {
        block_async_connect: true,
        strategy: ConnectStrategy::Fallback,
        url: None,
        connect_timeout: Some(Duration::from_secs(5)),
        retry_interval: None,
    }
}

/// Connect to a Kaspa node via Resolver or explicit URL.
///
/// If `rpc_url` is `None`, the Resolver automatically finds the best node.
/// This replaces the hardcoded `wss://photon-10.kaspa.red/...` URL.
pub async fn connect_client(
    network_id: NetworkId,
    rpc_url: Option<String>,
) -> Result<KaspaRpcClient, kaspa_wrpc_client::error::Error> {
    let url = if let Some(url) = &rpc_url {
        url.clone()
    } else {
        Resolver::default()
            .get_url(WrpcEncoding::Borsh, network_id)
            .await?
    };

    debug!("Connecting to Kaspad {}", url);
    let client = KaspaRpcClient::new_with_args(
        WrpcEncoding::Borsh,
        Some(&url),
        None,
        Some(network_id),
        None,
    )?;
    client.connect(Some(connect_options())).await.map_err(|e| {
        warn!("Kaspad connection failed: {e}");
        e
    })?;

    let server_info = client.get_server_info().await?;
    let connected_network = format!(
        "{}{}",
        server_info.network_id.network_type,
        server_info
            .network_id
            .suffix
            .map(|s| format!("-{}", s))
            .unwrap_or_default()
    );
    info!(
        "Connected to Kaspad {}, version: {}, network: {}",
        url, server_info.server_version, connected_network
    );

    if network_id != server_info.network_id {
        panic!(
            "Network mismatch, expected '{}', actual '{}'",
            network_id, connected_network
        );
    } else if !server_info.is_synced {
        let err_msg = format!("Kaspad {} is NOT synced", server_info.server_version);
        warn!("{err_msg}");
        Err(kaspa_wrpc_client::error::Error::Custom(err_msg))
    } else {
        Ok(client)
    }
}

/// Map of prefix → (pattern, engine_sender).
pub type EngineMap = HashMap<PrefixType, (PatternType, Sender<Msg>)>;

/// Polls the Kaspa virtual chain every ~1 second, filtering TXs by pattern
/// and forwarding matching payloads to the appropriate Engine.
///
/// This replaces the 5-second UTXO polling in `BlockchainWatcher`.
pub async fn run_listener(
    kaspad: KaspaRpcClient,
    engines: EngineMap,
    exit_signal: Arc<AtomicBool>,
) {
    let info = kaspad.get_block_dag_info().await.unwrap();
    let mut sink = info.sink;
    let mut now = Instant::now();
    info!("Listener started. Sink: {}", sink);

    loop {
        if exit_signal.load(Ordering::Relaxed) {
            info!("Exiting listener...");
            break;
        }
        sleep_until(now + Duration::from_secs(1)).await;
        now = Instant::now();

        let vcb = match kaspad.get_virtual_chain_from_block(sink, true).await {
            Ok(vcb) => vcb,
            Err(e) => {
                warn!("get_virtual_chain_from_block failed: {}", e);
                continue;
            }
        };

        debug!(
            "vspc: removed={}, accepted={}",
            vcb.removed_chain_block_hashes.len(),
            vcb.accepted_transaction_ids.len()
        );

        if let Some(new_sink) = vcb
            .accepted_transaction_ids
            .last()
            .map(|ncb| ncb.accepting_block_hash)
        {
            sink = new_sink;
        } else {
            continue;
        }

        // Handle reverted blocks
        for rcb in vcb.removed_chain_block_hashes {
            for (_, (_, sender)) in engines.iter() {
                let msg = Msg::BlkReverted {
                    accepting_hash: rcb,
                };
                let _ = sender.send(msg);
            }
        }

        // Iterate new chain blocks
        for ncb in vcb.accepted_transaction_ids {
            let accepting_hash = ncb.accepting_block_hash;

            // Filter TXs matching any engine's pattern (skip coinbase)
            let required_txs: Vec<Hash> = ncb
                .accepted_transaction_ids
                .iter()
                .copied()
                .skip(1)
                .filter(|&id| {
                    engines
                        .values()
                        .any(|(pattern, _)| check_pattern(id, pattern))
                })
                .collect();

            if required_txs.is_empty() {
                continue;
            }

            // Fetch the accepting block to get merge set
            let accepting_block = match kaspad.get_block(accepting_hash, false).await {
                Ok(blk) => blk,
                Err(e) => {
                    warn!("get_block failed for {}: {}", accepting_hash, e);
                    continue;
                }
            };

            let verbose = match accepting_block.verbose_data {
                Some(v) => v,
                None => continue,
            };

            // Collect payloads from merged blocks
            let mut required_payloads: HashMap<Hash, Option<Vec<u8>>> =
                required_txs.iter().map(|&id| (id, None)).collect();
            let mut required_num = required_payloads.len();

            'outer: for merged_hash in verbose
                .merge_set_blues_hashes
                .into_iter()
                .chain(verbose.merge_set_reds_hashes)
            {
                let merged_block = match kaspad.get_block(merged_hash, true).await {
                    Ok(blk) => blk,
                    Err(e) => {
                        warn!("get_block failed for merged {}: {}", merged_hash, e);
                        continue;
                    }
                };
                for tx in merged_block.transactions.into_iter().skip(1) {
                    if let Some(verbose_data) = &tx.verbose_data {
                        if let Some(required_payload) =
                            required_payloads.get_mut(&verbose_data.transaction_id)
                        {
                            if required_payload.is_none() {
                                required_payload.replace(tx.payload.clone());
                                required_num -= 1;
                                if required_num == 0 {
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }

            // Dispatch payloads to engines
            let mut consumed_txs = 0;
            for (&prefix, (pattern, sender)) in engines.iter() {
                let associated_txs: Vec<_> = required_txs
                    .iter()
                    .filter_map(|&id| {
                        if !check_pattern(id, pattern) {
                            return None;
                        }
                        match required_payloads.entry(id) {
                            Entry::Occupied(entry) => {
                                if Payload::check_header(entry.get().as_ref()?, prefix) {
                                    let payload = entry.remove().unwrap();
                                    consumed_txs += 1;
                                    return Some((id, Payload::strip_header(payload)));
                                }
                                None
                            }
                            Entry::Vacant(_) => None,
                        }
                    })
                    .collect();

                for (tx_id, _) in associated_txs.iter() {
                    info!("received episode tx: {}", tx_id);
                }

                if !associated_txs.is_empty() {
                    // We need the DAA score from the block header
                    let accepting_daa = accepting_block.header.daa_score;
                    let accepting_time = accepting_block.header.timestamp;
                    let msg = Msg::BlkAccepted {
                        accepting_hash,
                        accepting_daa,
                        accepting_time,
                        associated_txs,
                    };
                    let _ = sender.send(msg);
                }
                if consumed_txs == required_txs.len() {
                    break;
                }
            }
        }
    }

    // Signal all engines to exit
    for (_, (_, sender)) in engines.iter() {
        let _ = sender.send(Msg::Exit);
    }
}
