-- Free Play (1/2): e-mail + password identity on the existing `users` table (additive only).
--
-- `users` already carries email / password_hash / display_name / email_verified. Wallet and FACEIT
-- sign-ups fill email + password_hash with synthetic values, so a dedicated flag marks accounts
-- that really have an e-mail/password login. Nothing here touches existing rows' data.

ALTER TABLE users ADD COLUMN IF NOT EXISTS username              TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS has_password_login    BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified_at      TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_changed_at    TIMESTAMPTZ;

-- Newsletter consent (never required for transactional mail). Double-opt-in is a later step:
-- `newsletter_doi_confirmed_at` stays NULL until a confirmation flow exists, and no marketing
-- mail may be sent without it.
ALTER TABLE users ADD COLUMN IF NOT EXISTS newsletter_consent_at       TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS newsletter_consent_version  TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS newsletter_consent_source   TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS newsletter_revoked_at       TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS newsletter_doi_confirmed_at TIMESTAMPTZ;

-- Case-insensitive unique user names (only for accounts that have one).
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_username_lower
    ON users (lower(username)) WHERE username IS NOT NULL;

-- Case-insensitive unique e-mail. Created only if the existing data has no case-variant
-- duplicates, so this migration can never fail on production data; the application also
-- always looks e-mails up with lower(email).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM users GROUP BY lower(email) HAVING COUNT(*) > 1
    ) THEN
        CREATE UNIQUE INDEX IF NOT EXISTS uq_users_email_lower ON users (lower(email));
    ELSE
        RAISE NOTICE 'users contains case-variant duplicate e-mails; uq_users_email_lower NOT created';
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS email_verification_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,          -- SHA-256 hex; the plaintext token is never stored
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_email_verification_tokens_user ON email_verification_tokens (user_id);

CREATE TABLE IF NOT EXISTS password_reset_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_user ON password_reset_tokens (user_id);

-- Security audit trail. Never contains e-mail addresses, passwords or tokens; the IP is a
-- salted hash prefix, enough to correlate abuse but not to identify a person.
CREATE TABLE IF NOT EXISTS auth_audit_log (
    id         BIGSERIAL PRIMARY KEY,
    user_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    event      TEXT NOT NULL,
    ip_hash    TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_auth_audit_log_created ON auth_audit_log (created_at);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id);
