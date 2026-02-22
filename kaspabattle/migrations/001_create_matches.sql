CREATE TABLE IF NOT EXISTS matches (
    match_id        TEXT PRIMARY KEY,
    state           TEXT NOT NULL DEFAULT 'WaitingForOpponent',
    state_json      TEXT NOT NULL,
    player_a_id     TEXT NOT NULL,
    player_a_addr   TEXT NOT NULL,
    player_a_name   TEXT,
    player_b_id     TEXT,
    player_b_addr   TEXT,
    player_b_name   TEXT,
    wager_sompi     INTEGER NOT NULL,
    escrow_address  TEXT NOT NULL,
    game_type       TEXT NOT NULL DEFAULT 'cs2',
    timeout_at      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_matches_state ON matches(state);
CREATE INDEX IF NOT EXISTS idx_matches_escrow ON matches(escrow_address);
