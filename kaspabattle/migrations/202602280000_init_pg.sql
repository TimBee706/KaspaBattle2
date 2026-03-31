DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'match_status') THEN
        CREATE TYPE match_status AS ENUM (
            'OPEN',
            'AWAITING_FUNDING',
            'LOCKED',
            'IN_GAME',
            'RESOLVED',
            'CANCELLED'
        );
    END IF;
END $$;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'match_mode') THEN
        CREATE TYPE match_mode AS ENUM ('BO1', 'BO3');
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    kaspa_address VARCHAR(255) UNIQUE,
    faceit_id VARCHAR(255) UNIQUE,
    nickname VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    last_login_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE IF NOT EXISTS sessions (
    id VARCHAR(255) PRIMARY KEY,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS faceit_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE UNIQUE,
    faceit_player_id VARCHAR(255) UNIQUE NOT NULL,
    faceit_nickname VARCHAR(255) NOT NULL,
    faceit_elo INTEGER,
    faceit_skill_level INTEGER,
    faceit_avatar_url TEXT,
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at TIMESTAMP WITH TIME ZONE,
    linked_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    verified BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS faceit_stats_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    faceit_player_id VARCHAR(255) NOT NULL,
    game_id VARCHAR(255) NOT NULL,
    elo INTEGER NOT NULL,
    skill_level INTEGER NOT NULL,
    snapshot_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    onchain_match_id VARCHAR(255),
    creator_user_id UUID REFERENCES users(id) NOT NULL,
    opponent_user_id UUID REFERENCES users(id),
    game_id VARCHAR(255) NOT NULL,
    stake_kas BIGINT NOT NULL,
    mode match_mode NOT NULL,
    status match_status NOT NULL DEFAULT 'OPEN',
    external_match_id VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS match_results (
    match_id UUID PRIMARY KEY REFERENCES matches(id),
    winner_user_id UUID REFERENCES users(id),
    onchain_tx_hash VARCHAR(255),
    oracle_proof_hash VARCHAR(255)
);

CREATE INDEX IF NOT EXISTS idx_matches_status ON matches(status);
CREATE INDEX IF NOT EXISTS idx_matches_creator ON matches(creator_user_id);
CREATE INDEX IF NOT EXISTS idx_matches_opponent ON matches(opponent_user_id);