-- Migration 003: Escrows table and lifecycle tracking

CREATE TABLE IF NOT EXISTS escrows (
    challenge_id TEXT PRIMARY KEY,
    escrow_address TEXT NOT NULL UNIQUE,
    derivation_index INTEGER NOT NULL,
    wager_amount_sompi INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'NONE', -- NONE, PARTIAL, COMPLETE, PAID, REFUNDED
    payout_tx_id TEXT,
    refund_tx_a TEXT,
    refund_tx_b TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_escrows_status ON escrows(status);
CREATE INDEX IF NOT EXISTS idx_escrows_address ON escrows(escrow_address);

-- Add platform_address to matches or global config? 
-- For now we'll store it in a config table or env var.
CREATE TABLE IF NOT EXISTS platform_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO platform_config (key, value) VALUES ('platform_address', 'kaspatest:qplatform_placeholder');
