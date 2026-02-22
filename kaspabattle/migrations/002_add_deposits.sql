-- Deposits tracking table
CREATE TABLE IF NOT EXISTS deposits (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    match_id        TEXT NOT NULL REFERENCES matches(match_id),
    player_id       TEXT NOT NULL,
    tx_hash         TEXT,
    amount_sompi    INTEGER NOT NULL,
    detected_at     TEXT NOT NULL DEFAULT (datetime('now')),
    confirmed       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_deposits_match ON deposits(match_id);
CREATE INDEX IF NOT EXISTS idx_deposits_player ON deposits(player_id);

-- Additional columns for matches table
-- NOTE: Each ALTER TABLE must be executed as a separate statement
ALTER TABLE matches ADD COLUMN escrow_private_seed TEXT;
ALTER TABLE matches ADD COLUMN player_a_deposited INTEGER DEFAULT 0;
ALTER TABLE matches ADD COLUMN player_b_deposited INTEGER DEFAULT 0;
ALTER TABLE matches ADD COLUMN winner_id TEXT;
ALTER TABLE matches ADD COLUMN winner_address TEXT;
ALTER TABLE matches ADD COLUMN payout_tx_hash TEXT;
ALTER TABLE matches ADD COLUMN payout_amount_sompi INTEGER;
