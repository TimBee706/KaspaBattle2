CREATE TABLE IF NOT EXISTS wallet_login_challenges (
    id UUID PRIMARY KEY,
    kaspa_address TEXT NOT NULL,
    challenge_message TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_wallet_login_challenges_address
    ON wallet_login_challenges (kaspa_address);

CREATE INDEX IF NOT EXISTS idx_wallet_login_challenges_expires_at
    ON wallet_login_challenges (expires_at);
