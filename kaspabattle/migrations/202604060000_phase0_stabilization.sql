-- Phase 0: Stabilization Migration
-- Adds precise state-transition timestamps to the matches table,
-- replacing the created_at proxy used for timeout logic in MatchEpisode.
--
-- Affected timeout checks (match_episode.rs):
--   AWAITING_FUNDING: uses created_at (proxy) → replaced by awaiting_funding_since
--   GAME_ID_INPUT:    uses created_at (proxy) → replaced by game_id_input_since
--
-- These columns are optional (TIMESTAMPTZ, nullable) and set by the episode
-- runner at the moment of state transition for accurate timeout measurement.

ALTER TABLE matches
    ADD COLUMN IF NOT EXISTS awaiting_funding_since TIMESTAMPTZ;

ALTER TABLE matches
    ADD COLUMN IF NOT EXISTS game_id_input_since TIMESTAMPTZ;

-- Backfill for existing rows:
-- AWAITING_FUNDING rows: approximate with created_at (safe — no worse than before)
UPDATE matches
    SET awaiting_funding_since = created_at
    WHERE status = 'AWAITING_FUNDING'
      AND awaiting_funding_since IS NULL;

-- GAME_ID_INPUT rows: approximate with created_at
UPDATE matches
    SET game_id_input_since = created_at
    WHERE status = 'GAME_ID_INPUT'
      AND game_id_input_since IS NULL;

-- Index for the episode runner's timeout query (polls all active matches)
CREATE INDEX IF NOT EXISTS idx_matches_awaiting_funding_since
    ON matches (awaiting_funding_since)
    WHERE status = 'AWAITING_FUNDING';

CREATE INDEX IF NOT EXISTS idx_matches_game_id_input_since
    ON matches (game_id_input_since)
    WHERE status = 'GAME_ID_INPUT';
