-- Phase 4: FaceIT Watcher → Tournament Bracket Integration
-- Adds tournament linkage to faceit_watcher_jobs so the watcher can automatically
-- trigger the tournament oracle when bracket matches finish.
-- Also adds refund tracking columns for the tournament teams.

-- ─── faceit_watcher_jobs: Phase 4 columns ────────────────────────────────────

ALTER TABLE faceit_watcher_jobs
    ADD COLUMN IF NOT EXISTS tournament_bracket_slot_id UUID REFERENCES tournament_bracket(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS tournament_id UUID REFERENCES tournaments(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_faceit_watcher_jobs_slot
    ON faceit_watcher_jobs (tournament_bracket_slot_id)
    WHERE tournament_bracket_slot_id IS NOT NULL;

COMMENT ON COLUMN faceit_watcher_jobs.tournament_bracket_slot_id IS
    'When set, this watcher job monitors a tournament bracket slot match (not a 1v1 challenge match).';

COMMENT ON COLUMN faceit_watcher_jobs.tournament_id IS
    'Tournament this slot belongs to — denormalized for efficient querying.';

-- ─── tournament_teams: refund tracking ───────────────────────────────────────

ALTER TABLE tournament_teams
    ADD COLUMN IF NOT EXISTS refund_tx_hash TEXT,
    ADD COLUMN IF NOT EXISTS refund_executed_at TIMESTAMPTZ;

COMMENT ON COLUMN tournament_teams.refund_tx_hash IS
    'Kaspa TX ID of the refund transaction when the tournament is CANCELLED.';

-- ─── tournament_payments: unique constraint for idempotent processing ─────────

-- Used by the deposit watcher to ensure each UTXO is only processed once per tournament.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tournament_payments_tx_tournament
    ON tournament_payments (tx_id, tournament_id);

-- ─── tournament_bracket: dispute filing user tracking ────────────────────────

ALTER TABLE tournament_bracket
    ADD COLUMN IF NOT EXISTS dispute_filed_by UUID REFERENCES users(id) ON DELETE SET NULL;

COMMENT ON COLUMN tournament_bracket.dispute_filed_by IS
    'User ID of the captain who filed the bracket dispute.';

-- ─── tournaments: dispute tracking ───────────────────────────────────────────

ALTER TABLE tournaments
    ADD COLUMN IF NOT EXISTS dispute_filed_at TIMESTAMPTZ;

COMMENT ON COLUMN tournaments.dispute_filed_at IS
    'Timestamp when a captain filed a tournament-level dispute.';
