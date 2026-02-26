CREATE TABLE IF NOT EXISTS faceit_stats_snapshots (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    faceit_player_id TEXT NOT NULL,
    game_id TEXT NOT NULL,
    elo INTEGER NOT NULL,
    skill_level INTEGER NOT NULL,
    snapshot_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_faceit_stats_snapshots_player ON faceit_stats_snapshots(faceit_player_id);
CREATE INDEX IF NOT EXISTS idx_faceit_stats_snapshots_user ON faceit_stats_snapshots(user_id);