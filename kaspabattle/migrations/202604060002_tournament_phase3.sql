-- Phase 3: Bracket Execution, Payout, Dispute, Admin
-- Additive migration — no existing tables modified destructively.

-- ── 1. tournaments: add missing result/payout tracking columns ───────────────
ALTER TABLE tournaments
    ADD COLUMN IF NOT EXISTS winner_team_id_ref     UUID    REFERENCES tournament_teams(id),
    ADD COLUMN IF NOT EXISTS runner_up_team_id_ref  UUID    REFERENCES tournament_teams(id),
    ADD COLUMN IF NOT EXISTS payout_tx_hash         TEXT,
    ADD COLUMN IF NOT EXISTS payout_executed_at     TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS dispute_reason         TEXT,
    ADD COLUMN IF NOT EXISTS dispute_filed_at       TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS dispute_resolved_at    TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS dispute_resolved_by    UUID    REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS registration_deadline  TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS cancelled_reason       TEXT;

-- ── 2. tournament_bracket: add review/dispute columns per slot ────────────────
ALTER TABLE tournament_bracket
    ADD COLUMN IF NOT EXISTS disputed           BOOLEAN     DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS dispute_reason     TEXT,
    ADD COLUMN IF NOT EXISTS dispute_filed_by   UUID        REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS dispute_filed_at   TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS dispute_resolved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS resolved_by_admin  UUID        REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS reported_score     TEXT;

-- ── 3. tournament_payments: add confirmed_at column ──────────────────────────
ALTER TABLE tournament_payments
    ADD COLUMN IF NOT EXISTS confirmed_at       TIMESTAMPTZ;

-- ── 4. audit_log: universal audit trail for all admin actions ────────────────
CREATE TABLE IF NOT EXISTS audit_log (
    id              UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type     TEXT            NOT NULL,   -- 'tournament', 'bracket_slot', 'match', etc.
    entity_id       UUID            NOT NULL,
    action          TEXT            NOT NULL,   -- 'dispute_filed', 'dispute_resolved', 'force_cancel', etc.
    actor_user_id   UUID            REFERENCES users(id),
    actor_role      TEXT,                       -- 'admin', 'organizer', 'captain', 'system'
    details         JSONB,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_log_entity
    ON audit_log(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor
    ON audit_log(actor_user_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_created
    ON audit_log(created_at DESC);

-- ── 5. tournament_bracket: winner attribution for FaceIT-watcher ──────────────
-- faceit_faction_a / faceit_faction_b track which FaceIT faction each team is
-- (faction1, faction2). Set when a match is created/submitted.
ALTER TABLE tournament_bracket
    ADD COLUMN IF NOT EXISTS faceit_faction_a   TEXT,   -- 'faction1' or 'faction2'
    ADD COLUMN IF NOT EXISTS faceit_faction_b   TEXT;

-- ── 6. faceit_watcher_jobs: add tournament_bracket_slot_id for bracket matches ─
-- Allows the FaceIT watcher to process bracket slot results in addition to
-- regular 1v1 match results. NULL = regular match, NOT NULL = bracket slot.
ALTER TABLE faceit_watcher_jobs
    ADD COLUMN IF NOT EXISTS tournament_bracket_slot_id UUID REFERENCES tournament_bracket(id),
    ADD COLUMN IF NOT EXISTS tournament_id              UUID REFERENCES tournaments(id);

CREATE INDEX IF NOT EXISTS idx_faceit_watcher_bracket_slot
    ON faceit_watcher_jobs(tournament_bracket_slot_id)
    WHERE tournament_bracket_slot_id IS NOT NULL;

-- ── 7. tournament_teams: add refund tracking ──────────────────────────────────
ALTER TABLE tournament_teams
    ADD COLUMN IF NOT EXISTS refund_tx_hash         TEXT,
    ADD COLUMN IF NOT EXISTS refund_executed_at     TIMESTAMPTZ;
