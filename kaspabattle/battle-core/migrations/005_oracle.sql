-- Migration 005: Oracle Jobs
CREATE TABLE IF NOT EXISTS oracle_jobs (
    job_id TEXT PRIMARY KEY,
    match_id TEXT NOT NULL REFERENCES matches(match_id),
    faceit_match_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(
        status IN (
            'Pending',
            'Processing',
            'Resolved',
            'Failed',
            'Cancelled'
        )
    ),
    reported_winner TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_oracle_jobs_match ON oracle_jobs(match_id);
CREATE INDEX IF NOT EXISTS idx_oracle_jobs_status ON oracle_jobs(status);