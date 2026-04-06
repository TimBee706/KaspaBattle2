-- Phase 1: Tournament Schema
-- Adds all tables required for 5v5 tournaments, team management,
-- bracket execution, and prize-pool tracking.
--
-- Design principles:
--   - Fully additive: no existing tables are modified
--   - Idempotent: all statements use IF NOT EXISTS / DO $$ guards
--   - Parallel to V1: existing matches/users tables are referenced, not changed
--   - Single escrow per tournament (not per team) — attribution via tournament_payments

-- ─── Enums ────────────────────────────────────────────────────────────────────

DO $$ BEGIN
    CREATE TYPE tournament_status AS ENUM (
        'REGISTRATION',       -- Open for team registration
        'FUNDED',             -- All teams have confirmed their deposits
        'BRACKET_READY',      -- Bracket generated and locked by organizer
        'IN_PROGRESS',        -- Matches are being played
        'COMPLETED',          -- Winner determined, payout executed
        'CANCELLED',          -- Cancelled before start (refunds issued)
        'DISPUTED'            -- Admin intervention required
    );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE team_deposit_status AS ENUM (
        'PENDING',            -- Buy-in not yet received
        'CONFIRMED',          -- Buy-in confirmed on-chain
        'REFUNDED'            -- Buy-in refunded (tournament cancelled)
    );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE bracket_slot_status AS ENUM (
        'WAITING',            -- Waiting for both teams
        'READY',              -- Both teams assigned, match can start
        'IN_PROGRESS',        -- FaceIT match in progress
        'COMPLETED',          -- Result confirmed
        'BYE',                -- One team auto-advances (odd bracket)
        'CANCELLED'           -- Slot cancelled (e.g. team withdrew)
    );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- ─── tournaments ─────────────────────────────────────────────────────────────
-- One row per tournament. The escrow_address holds all team buy-ins.

CREATE TABLE IF NOT EXISTS tournaments (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Basic info
    title                   TEXT NOT NULL,
    game_type               TEXT NOT NULL DEFAULT 'CS2',
    max_teams               INTEGER NOT NULL DEFAULT 8
                                CHECK (max_teams IN (4, 8, 16)),

    -- Financial
    buy_in_sompi            BIGINT NOT NULL CHECK (buy_in_sompi > 0),
    prize_winner_pct        SMALLINT NOT NULL DEFAULT 70  -- % of prize pool to 1st place
                                CHECK (prize_winner_pct BETWEEN 1 AND 100),
    prize_runner_up_pct     SMALLINT NOT NULL DEFAULT 20  -- % to 2nd place
                                CHECK (prize_runner_up_pct BETWEEN 0 AND 99),
    platform_fee_pct        SMALLINT NOT NULL DEFAULT 10  -- platform fee %
                                CHECK (platform_fee_pct BETWEEN 0 AND 50),
    -- Constraint: splits must add to 100
    CONSTRAINT pct_sum_check CHECK (
        prize_winner_pct + prize_runner_up_pct + platform_fee_pct = 100
    ),

    -- Escrow
    escrow_address          TEXT UNIQUE,                  -- Single Kaspa address for all deposits
    total_prize_pool_sompi  BIGINT NOT NULL DEFAULT 0,    -- Running sum of confirmed deposits

    -- Lifecycle
    status                  tournament_status NOT NULL DEFAULT 'REGISTRATION',
    registration_deadline   TIMESTAMPTZ,                  -- NULL = no deadline

    -- Payout tracking
    payout_tx_hash          TEXT,
    payout_executed_at      TIMESTAMPTZ,
    refund_tx_hash          TEXT,

    -- Relations
    organizer_user_id       UUID NOT NULL REFERENCES users(id),
    winner_team_id          UUID,                         -- FK added after team table exists (below)
    runner_up_team_id       UUID,

    -- Metadata
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ─── tournament_teams ────────────────────────────────────────────────────────
-- Each team in a tournament. Captain is the team creator.

CREATE TABLE IF NOT EXISTS tournament_teams (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id           UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,

    -- Team info
    name                    TEXT NOT NULL,
    captain_user_id         UUID NOT NULL REFERENCES users(id),

    -- FaceIT integration (team-level, optional for MVP)
    faceit_team_id          TEXT,

    -- Deposit tracking
    deposit_status          team_deposit_status NOT NULL DEFAULT 'PENDING',
    deposit_tx_hash         TEXT,                         -- TX that funded this team's buy-in
    deposit_confirmed_at    TIMESTAMPTZ,

    -- Seeding (set by organizer before bracket lock)
    seed                    INTEGER,

    -- Metadata
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (tournament_id, name),                         -- No duplicate team names per tournament
    UNIQUE (tournament_id, captain_user_id)               -- One team per captain per tournament
);

-- Add winner/runner-up FKs now that tournament_teams exists
ALTER TABLE tournaments
    ADD COLUMN IF NOT EXISTS winner_team_id_ref     UUID REFERENCES tournament_teams(id),
    ADD COLUMN IF NOT EXISTS runner_up_team_id_ref  UUID REFERENCES tournament_teams(id);

-- Drop temporary un-constrained columns if they exist (idempotent cleanup)
ALTER TABLE tournaments DROP COLUMN IF EXISTS winner_team_id;
ALTER TABLE tournaments DROP COLUMN IF EXISTS runner_up_team_id;

-- ─── team_members ────────────────────────────────────────────────────────────
-- Individual players on a team (5 per team for 5v5).

CREATE TABLE IF NOT EXISTS team_members (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id                 UUID NOT NULL REFERENCES tournament_teams(id) ON DELETE CASCADE,
    user_id                 UUID NOT NULL REFERENCES users(id),

    -- FaceIT identity for this player
    faceit_player_id        TEXT,

    -- Role within team
    is_captain              BOOLEAN NOT NULL DEFAULT FALSE,

    -- Metadata
    joined_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (team_id, user_id)                             -- A user can only be on one team slot
);

-- ─── tournament_bracket ──────────────────────────────────────────────────────
-- One row per match slot in the bracket.
-- Single-Elimination: round 1 has N/2 slots, round 2 has N/4, etc.

CREATE TABLE IF NOT EXISTS tournament_bracket (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id           UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,

    -- Bracket position
    round                   INTEGER NOT NULL CHECK (round >= 1),  -- 1 = first round, 2 = quarters, etc.
    slot_index              INTEGER NOT NULL CHECK (slot_index >= 0), -- Position within round (0-based)

    -- Teams
    team_a_id               UUID REFERENCES tournament_teams(id),
    team_b_id               UUID REFERENCES tournament_teams(id),
    winner_team_id          UUID REFERENCES tournament_teams(id),

    -- FaceIT match linking (MVP: captain submits match ID manually)
    faceit_match_id         TEXT,                         -- Linked FaceIT match
    faceit_match_status     TEXT,                         -- Last known FaceIT status

    -- Status
    status                  bracket_slot_status NOT NULL DEFAULT 'WAITING',

    -- Timestamps
    match_started_at        TIMESTAMPTZ,
    match_finished_at       TIMESTAMPTZ,

    -- Link to watcher job (reuses existing faceit_watcher_jobs infrastructure)
    watcher_job_id          UUID REFERENCES faceit_watcher_jobs(id),

    -- Metadata
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (tournament_id, round, slot_index)             -- Each bracket position is unique
);

-- ─── tournament_payments ────────────────────────────────────────────────────
-- Tracks individual UTXO deposits into the tournament escrow address.
-- Mirrors the existing `payments` table pattern for 1v1 matches.

CREATE TABLE IF NOT EXISTS tournament_payments (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id           UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    team_id                 UUID REFERENCES tournament_teams(id),

    -- UTXO info
    tx_id                   TEXT NOT NULL,
    amount_sompi            BIGINT NOT NULL,
    block_daa_score         BIGINT NOT NULL DEFAULT 0,
    confirmations           INTEGER NOT NULL DEFAULT 0,

    -- Attribution (sender address resolved from TX or submitted by captain)
    sender_kaspa_address    TEXT,

    -- Metadata
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (tx_id, tournament_id)                         -- Idempotent: same TX not counted twice
);

-- ─── Indices ─────────────────────────────────────────────────────────────────

CREATE INDEX IF NOT EXISTS idx_tournaments_status
    ON tournaments (status);

CREATE INDEX IF NOT EXISTS idx_tournaments_organizer
    ON tournaments (organizer_user_id);

CREATE INDEX IF NOT EXISTS idx_tournament_teams_tournament
    ON tournament_teams (tournament_id);

CREATE INDEX IF NOT EXISTS idx_tournament_teams_captain
    ON tournament_teams (captain_user_id);

CREATE INDEX IF NOT EXISTS idx_team_members_team
    ON team_members (team_id);

CREATE INDEX IF NOT EXISTS idx_team_members_user
    ON team_members (user_id);

CREATE INDEX IF NOT EXISTS idx_tournament_bracket_tournament
    ON tournament_bracket (tournament_id);

CREATE INDEX IF NOT EXISTS idx_tournament_bracket_status
    ON tournament_bracket (status)
    WHERE status IN ('READY', 'IN_PROGRESS');

CREATE INDEX IF NOT EXISTS idx_tournament_payments_tournament
    ON tournament_payments (tournament_id);

CREATE INDEX IF NOT EXISTS idx_tournament_payments_team
    ON tournament_payments (team_id);

CREATE INDEX IF NOT EXISTS idx_tournament_payments_tx
    ON tournament_payments (tx_id);
