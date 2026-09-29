-- Native Games (Phase 1a): explicit game provider + games catalog.
--
-- Additive only. Existing rows keep working: every pre-existing match is a FACEIT match.
-- NOTE: new match_status enum values live in 202609290002 (a new enum value cannot be
-- used in the transaction that adds it).

-- ── Games catalog ────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS games (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug             TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    provider         TEXT NOT NULL CHECK (provider IN ('FACEIT', 'NATIVE')),
    player_count     INTEGER NOT NULL DEFAULT 2 CHECK (player_count >= 2),
    requires_faceit  BOOLEAN NOT NULL,
    native_game_type TEXT,
    enabled          BOOLEAN NOT NULL DEFAULT TRUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT games_provider_consistency CHECK (
        (provider = 'FACEIT' AND requires_faceit = TRUE  AND native_game_type IS NULL) OR
        (provider = 'NATIVE' AND requires_faceit = FALSE AND native_game_type IS NOT NULL)
    )
);

-- Existing FACEIT games (slugs equal the legacy free-text matches.game_id values).
INSERT INTO games (slug, name, provider, player_count, requires_faceit, native_game_type) VALUES
    ('cs2',           'Counter-Strike 2', 'FACEIT', 2, TRUE, NULL),
    ('dota2',         'Dota 2',           'FACEIT', 2, TRUE, NULL),
    ('valorant',      'Valorant',         'FACEIT', 2, TRUE, NULL),
    ('rocket_league', 'Rocket League',    'FACEIT', 2, TRUE, NULL),
    ('lol',           'League of Legends','FACEIT', 2, TRUE, NULL)
ON CONFLICT (slug) DO NOTHING;

-- First native browser game.
INSERT INTO games (slug, name, provider, player_count, requires_faceit, native_game_type) VALUES
    ('connect-four', 'Vier Gewinnt', 'NATIVE', 2, FALSE, 'CONNECT_FOUR')
ON CONFLICT (slug) DO NOTHING;

-- ── Provider snapshot on matches ─────────────────────────────────────────────
-- Copied from the games catalog at creation time, so later catalog edits never
-- change an existing match. DEFAULT 'FACEIT' backfills all legacy rows.
ALTER TABLE matches ADD COLUMN IF NOT EXISTS provider         TEXT    NOT NULL DEFAULT 'FACEIT';
ALTER TABLE matches ADD COLUMN IF NOT EXISTS requires_faceit  BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS native_game_type TEXT;

-- Settlement metadata for natively decided matches.
ALTER TABLE matches ADD COLUMN IF NOT EXISTS result_source    TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS game_finished_at TIMESTAMPTZ;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS result_hash      TEXT;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'matches_provider_check') THEN
        ALTER TABLE matches ADD CONSTRAINT matches_provider_check
            CHECK (provider IN ('FACEIT', 'NATIVE'));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'matches_provider_consistency') THEN
        ALTER TABLE matches ADD CONSTRAINT matches_provider_consistency
            CHECK (provider = 'FACEIT' OR (native_game_type IS NOT NULL AND requires_faceit = FALSE));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'matches_result_source_check') THEN
        ALTER TABLE matches ADD CONSTRAINT matches_result_source_check
            CHECK (result_source IS NULL OR result_source IN ('FACEIT', 'NATIVE_ENGINE'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_matches_provider ON matches (provider);
