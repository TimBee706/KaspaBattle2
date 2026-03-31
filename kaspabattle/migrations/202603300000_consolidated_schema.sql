-- One-time migration: add escrow_address column if missing
ALTER TABLE matches ADD COLUMN IF NOT EXISTS escrow_address TEXT;

-- v0.2 migrations: deposit tracking + FaceID
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_deposit_tx_hash TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_deposit_tx_hash TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_deposit_confirmed BOOLEAN DEFAULT FALSE;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_deposit_confirmed BOOLEAN DEFAULT FALSE;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_faceid_hash TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_faceid_hash TEXT;

-- v0.3 migrations: FACEIT cache columns
ALTER TABLE faceit_links ADD COLUMN IF NOT EXISTS faceit_elo INTEGER;
ALTER TABLE faceit_links ADD COLUMN IF NOT EXISTS faceit_skill_level INTEGER;
ALTER TABLE faceit_links ADD COLUMN IF NOT EXISTS faceit_cache_updated_at TIMESTAMPTZ;

-- v0.4 migrations: payment detection system
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'FUNDED') THEN ALTER TYPE match_status ADD VALUE 'FUNDED'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'PAID_OUT') THEN ALTER TYPE match_status ADD VALUE 'PAID_OUT'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'DISPUTED') THEN ALTER TYPE match_status ADD VALUE 'DISPUTED'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'RESOLVING') THEN ALTER TYPE match_status ADD VALUE 'RESOLVING'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'IN_GAME') THEN ALTER TYPE match_status ADD VALUE 'IN_GAME'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'DRAFT') THEN ALTER TYPE match_status ADD VALUE 'DRAFT'; END IF; END $$;

CREATE TABLE IF NOT EXISTS payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    player_id UUID REFERENCES users(id),
    player_role TEXT NOT NULL CHECK (player_role IN ('A', 'B')),
    tx_id TEXT NOT NULL,
    amount_sompi BIGINT NOT NULL,
    block_daa_score BIGINT NOT NULL DEFAULT 0,
    confirmations INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tx_id, match_id)
);

CREATE TABLE IF NOT EXISTS wallet_login_challenges (
    id UUID PRIMARY KEY,
    kaspa_address TEXT NOT NULL,
    challenge_message TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE matches ADD COLUMN IF NOT EXISTS wager_amount_sompi BIGINT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_deposit_amount_sompi BIGINT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_deposit_amount_sompi BIGINT;

DELETE FROM payments p1 USING payments p2 WHERE p1.ctid < p2.ctid AND p1.match_id = p2.match_id AND p1.player_role = p2.player_role;

ALTER TABLE payments DROP CONSTRAINT IF EXISTS payments_tx_id_match_id_key;
DROP INDEX IF EXISTS uq_payments_match_player_role;
DROP INDEX IF EXISTS payments_tx_id_match_id_idx;
CREATE UNIQUE INDEX IF NOT EXISTS uq_payments_match_role ON payments (match_id, player_role);

DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'GAME_ID_INPUT') THEN ALTER TYPE match_status ADD VALUE 'GAME_ID_INPUT'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'FINISHED_FACEIT') THEN ALTER TYPE match_status ADD VALUE 'FINISHED_FACEIT'; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'READY_FOR_PAYOUT') THEN ALTER TYPE match_status ADD VALUE 'READY_FOR_PAYOUT'; END IF; END $$;

ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_match_id_player_a TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_match_id_player_b TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_match_id_final TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_match_status TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_finished_at TIMESTAMPTZ;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_winner_faction TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS faceit_score TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS winner_user_id UUID REFERENCES users(id);
ALTER TABLE matches ADD COLUMN IF NOT EXISTS loser_user_id UUID REFERENCES users(id);
ALTER TABLE matches ADD COLUMN IF NOT EXISTS payout_pskt_hex TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS payout_tx_hash TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS payout_status TEXT;

CREATE TABLE IF NOT EXISTS faceit_watcher_jobs (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id            UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    faceit_match_id     TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'ACTIVE',
    retry_count         INTEGER NOT NULL DEFAULT 0,
    max_retries         INTEGER NOT NULL DEFAULT 360,
    next_poll_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_polled_at      TIMESTAMPTZ,
    last_faceit_status  TEXT,
    error_message       TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(match_id)
);

CREATE TABLE IF NOT EXISTS multisig_escrows (
    match_id                UUID PRIMARY KEY REFERENCES matches(id) ON DELETE CASCADE,
    pubkey_a_hex            TEXT NOT NULL,
    pubkey_b_hex            TEXT NOT NULL,
    pubkey_oracle_hex       TEXT NOT NULL,
    redeem_script_hex       TEXT NOT NULL,
    p2sh_address            TEXT NOT NULL UNIQUE,
    wager_per_player_sompi  BIGINT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'CREATED',
    timelock_timestamp      BIGINT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_multisig_escrows_address ON multisig_escrows (p2sh_address);

-- Refactoring N-1: rename stake_kas to wager_sompi (idempotent)
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'matches' AND column_name = 'stake_kas'
    ) THEN
        ALTER TABLE matches RENAME COLUMN stake_kas TO wager_sompi;
    END IF;
END $$;
