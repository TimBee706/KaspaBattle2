//! Engine integration test — validates the full BattleEpisode lifecycle
//! through the Engine without any network or database.
//!
//! Simulates: NewEpisode → CreateMatch → JoinMatch → ConfirmDeposit×2
//! → ReportResult → InitiatePayout, plus a DAG-reorg rollback scenario.

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use kaspa_hashes::Hash;

    use crate::commands::{BattleCommand, GameType};
    use crate::episode::BattleEpisode;
    use crate::kdapp_engine::{Engine, EngineMsg, EpisodeMessage};
    use crate::kdapp_episode::{EpisodeEventHandler, EpisodeId, PayloadMetadata};
    use crate::kdapp_pki::{generate_keypair, PubKey};

    /// A no-op handler that just records events for assertions.
    #[derive(Default)]
    struct TestHandler {
        events: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl TestHandler {
        fn new() -> Self {
            Self {
                events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EpisodeEventHandler<BattleEpisode> for TestHandler {
        fn on_initialize(&self, episode_id: EpisodeId, _episode: &BattleEpisode) {
            self.events.lock().unwrap().push(format!("init:{}", episode_id));
        }

        fn on_command(
            &self,
            episode_id: EpisodeId,
            _episode: &BattleEpisode,
            cmd: &BattleCommand,
            _authorization: Option<PubKey>,
            _metadata: &PayloadMetadata,
        ) {
            self.events.lock().unwrap().push(format!("cmd:{}:{:?}", episode_id, cmd));
        }

        fn on_rollback(&self, episode_id: EpisodeId, _episode: &BattleEpisode) {
            self.events.lock().unwrap().push(format!("rollback:{}", episode_id));
        }
    }

    /// Helper: create a test metadata block for a given DAA score.
    fn make_metadata(daa: u64) -> PayloadMetadata {
        PayloadMetadata {
            accepting_hash: Hash::from_bytes([daa as u8; 32]),
            accepting_daa: daa,
            accepting_time: 1700000000 + daa,
            tx_id: Hash::from_bytes([(daa + 100) as u8; 32]),
        }
    }

    /// Helper: serialize an EpisodeMessage into bytes for TX payload.
    fn serialize_msg(msg: &EpisodeMessage<BattleEpisode>) -> Vec<u8> {
        borsh::to_vec(msg).expect("serialize failed")
    }

    /// Helper: send a BlkAccepted with one TX to the engine.
    fn send_block(
        tx: &mpsc::Sender<EngineMsg>,
        daa: u64,
        payloads: Vec<(Hash, Vec<u8>)>,
    ) {
        let meta = make_metadata(daa);
        tx.send(EngineMsg::BlkAccepted {
            accepting_hash: meta.accepting_hash,
            accepting_daa: meta.accepting_daa,
            accepting_time: meta.accepting_time,
            associated_txs: payloads,
        })
        .unwrap();
    }

    #[test]
    fn test_full_lifecycle_via_engine() {
        let (tx, rx) = mpsc::channel();
        let handler = TestHandler::new();
        let events = handler.events.clone();

        // Start engine on a background thread
        let engine_thread = std::thread::spawn(move || {
            let mut engine = Engine::<BattleEpisode, TestHandler>::new(rx);
            engine.start(vec![handler]);
        });

        let (sk_a, pk_a) = generate_keypair();
        let (sk_b, pk_b) = generate_keypair();
        let episode_id: EpisodeId = 42;

        // 1. NewEpisode
        let new_ep = EpisodeMessage::<BattleEpisode>::NewEpisode {
            episode_id,
            participants: vec![pk_a, pk_b],
        };
        let payload = serialize_msg(&new_ep);
        let tx_hash = Hash::from_bytes([1u8; 32]);
        send_block(&tx, 100, vec![(tx_hash, payload)]);

        // Small sleep to let Engine process
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 2. CreateMatch (signed by player A)
        let create_cmd = BattleCommand::CreateMatch {
            game_type: GameType::CS2,
            wager_sompi: 5_000_000_000,
        };
        let create_msg = EpisodeMessage::new_signed_command(episode_id, create_cmd, sk_a, pk_a);
        let payload = serialize_msg(&create_msg);
        let tx_hash = Hash::from_bytes([2u8; 32]);
        send_block(&tx, 101, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 3. JoinMatch (signed by player B)
        let join_msg = EpisodeMessage::new_signed_command(episode_id, BattleCommand::JoinMatch, sk_b, pk_b);
        let payload = serialize_msg(&join_msg);
        let tx_hash = Hash::from_bytes([3u8; 32]);
        send_block(&tx, 102, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 4. ConfirmDeposit A (signed by player A)
        let dep_a = BattleCommand::ConfirmDeposit {
            tx_hash: [4; 32],
            amount_sompi: 5_000_000_000,
        };
        let dep_a_msg = EpisodeMessage::new_signed_command(episode_id, dep_a, sk_a, pk_a);
        let payload = serialize_msg(&dep_a_msg);
        let tx_hash = Hash::from_bytes([4u8; 32]);
        send_block(&tx, 103, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 5. ConfirmDeposit B (signed by player B)
        let dep_b = BattleCommand::ConfirmDeposit {
            tx_hash: [5; 32],
            amount_sompi: 5_000_000_000,
        };
        let dep_b_msg = EpisodeMessage::new_signed_command(episode_id, dep_b, sk_b, pk_b);
        let payload = serialize_msg(&dep_b_msg);
        let tx_hash = Hash::from_bytes([5u8; 32]);
        send_block(&tx, 104, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 6. ReportResult (unsigned — from oracle)
        let result_cmd = BattleCommand::ReportResult {
            winner_pubkey: pk_a,
            faceit_match_id: [0; 32],
            score_a: 16,
            score_b: 12,
        };
        let result_msg = EpisodeMessage::new_signed_command(episode_id, result_cmd, sk_a, pk_a);
        let payload = serialize_msg(&result_msg);
        let tx_hash = Hash::from_bytes([6u8; 32]);
        send_block(&tx, 105, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 7. InitiatePayout (unsigned — from system)
        let payout_msg = EpisodeMessage::<BattleEpisode>::UnsignedCommand {
            episode_id,
            cmd: BattleCommand::InitiatePayout,
        };
        let payload = serialize_msg(&payout_msg);
        let tx_hash = Hash::from_bytes([7u8; 32]);
        send_block(&tx, 106, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Exit engine
        tx.send(EngineMsg::Exit).unwrap();
        engine_thread.join().unwrap();

        // Assert all events were recorded
        let recorded = events.lock().unwrap();
        assert!(recorded.iter().any(|e| e.starts_with("init:42")), "Should have init event");
        assert!(recorded.iter().any(|e| e.contains("CreateMatch")), "Should have CreateMatch");
        assert!(recorded.iter().any(|e| e.contains("JoinMatch")), "Should have JoinMatch");
        assert!(recorded.iter().filter(|e| e.contains("ConfirmDeposit")).count() == 2, "Should have 2 deposits");
        assert!(recorded.iter().any(|e| e.contains("ReportResult")), "Should have ReportResult");
        assert!(recorded.iter().any(|e| e.contains("InitiatePayout")), "Should have InitiatePayout");
    }

    #[test]
    fn test_dag_reorg_rollback() {
        let (tx, rx) = mpsc::channel();
        let handler = TestHandler::new();
        let events = handler.events.clone();

        let engine_thread = std::thread::spawn(move || {
            let mut engine = Engine::<BattleEpisode, TestHandler>::new(rx);
            engine.start(vec![handler]);
        });

        let (sk_a, pk_a) = generate_keypair();
        let (sk_b, pk_b) = generate_keypair();
        let episode_id: EpisodeId = 99;

        // 1. Create episode
        let new_ep = EpisodeMessage::<BattleEpisode>::NewEpisode {
            episode_id,
            participants: vec![pk_a, pk_b],
        };
        let payload = serialize_msg(&new_ep);
        let tx_hash = Hash::from_bytes([10u8; 32]);
        send_block(&tx, 200, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 2. CreateMatch
        let create_cmd = BattleCommand::CreateMatch {
            game_type: GameType::CS2,
            wager_sompi: 1_000_000_000,
        };
        let create_msg = EpisodeMessage::new_signed_command(episode_id, create_cmd, sk_a, pk_a);
        let payload = serialize_msg(&create_msg);
        let tx_hash = Hash::from_bytes([11u8; 32]);
        let block_hash_to_revert = make_metadata(201).accepting_hash;
        send_block(&tx, 201, vec![(tx_hash, payload)]);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 3. DAG reorg — revert the CreateMatch block
        tx.send(EngineMsg::BlkReverted {
            accepting_hash: block_hash_to_revert,
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Exit
        tx.send(EngineMsg::Exit).unwrap();
        engine_thread.join().unwrap();

        // Assert rollback was recorded
        let recorded = events.lock().unwrap();
        assert!(recorded.iter().any(|e| e.starts_with("init:99")), "Should have init");
        assert!(recorded.iter().any(|e| e.contains("CreateMatch")), "Should have CreateMatch");
        assert!(recorded.iter().any(|e| e.starts_with("rollback:99")), "Should have rollback event");
    }
}
