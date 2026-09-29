-- Persistent, single-use, short-lived FACEIT OAuth state (PKCE verifier + CSRF state).
-- The state value itself is never stored, only its SHA-256 hash.
CREATE TABLE IF NOT EXISTS faceit_oauth_states (
    state_hash    TEXT PRIMARY KEY,
    code_verifier TEXT NOT NULL,
    user_id       UUID,
    return_to     TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at    TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_faceit_oauth_states_expires_at ON faceit_oauth_states (expires_at);
