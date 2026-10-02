-- Free Play (2/2): off-chain, wallet-free, stake-free games.
--
-- Free Play lives in its own tables on purpose: there is no foreign key to `matches`,
-- `multisig_escrows` or `payments`, so a free-play game structurally cannot own an escrow,
-- a deposit or a payout. The CHECKs below make that explicit and tamper-proof.

-- Game mode is a real domain field on paid matches too: existing rows are `kaspa_testnet`,
-- and a paid match can never be `free_play` (those go through /free-play only).
ALTER TABLE matches ADD COLUMN IF NOT EXISTS game_mode TEXT NOT NULL DEFAULT 'kaspa_testnet';
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'matches_game_mode_check') THEN
        ALTER TABLE matches ADD CONSTRAINT matches_game_mode_check CHECK (game_mode = 'kaspa_testnet');
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS free_play_games (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mode                 TEXT NOT NULL DEFAULT 'free_play' CHECK (mode = 'free_play'),
    game_type            TEXT NOT NULL DEFAULT 'connect_four' CHECK (game_type = 'connect_four'),
    opponent_kind        TEXT NOT NULL CHECK (opponent_kind IN ('human', 'bot')),
    bot_difficulty       TEXT CHECK (bot_difficulty IN ('easy', 'medium', 'hard')),
    status               TEXT NOT NULL CHECK (status IN ('open', 'active', 'finished', 'cancelled')),
    player_one_id        UUID NOT NULL REFERENCES users(id),
    player_two_id        UUID REFERENCES users(id),              -- NULL for open lobbies and bot games
    -- Money is structurally impossible: always zero, no address, no escrow.
    stake_sompi          BIGINT NOT NULL DEFAULT 0 CHECK (stake_sompi = 0),
    board_state          JSONB NOT NULL,
    current_slot         SMALLINT CHECK (current_slot IN (1, 2)),
    move_count           INTEGER NOT NULL DEFAULT 0 CHECK (move_count BETWEEN 0 AND 42),
    version              BIGINT NOT NULL DEFAULT 0,
    state_hash           TEXT NOT NULL,
    last_move_column     SMALLINT CHECK (last_move_column BETWEEN 0 AND 6),
    last_move_row        SMALLINT CHECK (last_move_row BETWEEN 0 AND 5),
    winner_slot          SMALLINT CHECK (winner_slot IN (1, 2)),
    winner_user_id       UUID REFERENCES users(id),               -- NULL for draws and bot wins
    result               TEXT CHECK (result IN ('win', 'draw', 'abandoned')),
    end_reason           TEXT CHECK (end_reason IN ('connect_four', 'draw', 'resignation', 'timeout', 'left')),
    turn_deadline        TIMESTAMPTZ,
    rematch_requested_by UUID REFERENCES users(id),
    rematch_game_id      UUID REFERENCES free_play_games(id),
    rematch_of           UUID REFERENCES free_play_games(id),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    joined_at            TIMESTAMPTZ,
    started_at           TIMESTAMPTZ,
    finished_at          TIMESTAMPTZ,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fp_distinct_players CHECK (player_two_id IS NULL OR player_two_id <> player_one_id),
    CONSTRAINT fp_human_shape CHECK (
        opponent_kind <> 'human' OR (bot_difficulty IS NULL AND (status IN ('open', 'cancelled') OR player_two_id IS NOT NULL))
    ),
    CONSTRAINT fp_bot_shape CHECK (
        opponent_kind <> 'bot' OR (bot_difficulty IS NOT NULL AND player_two_id IS NULL)
    ),
    CONSTRAINT fp_finished_shape CHECK (
        (status = 'finished') = (result IS NOT NULL AND end_reason IS NOT NULL AND finished_at IS NOT NULL)
        OR status = 'cancelled'
    )
);
CREATE INDEX IF NOT EXISTS idx_fp_games_open ON free_play_games (created_at) WHERE status = 'open';
CREATE INDEX IF NOT EXISTS idx_fp_games_p1 ON free_play_games (player_one_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_fp_games_p2 ON free_play_games (player_two_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_fp_games_deadline ON free_play_games (turn_deadline) WHERE status = 'active';

CREATE TABLE IF NOT EXISTS free_play_moves (
    id              BIGSERIAL PRIMARY KEY,
    game_id         UUID NOT NULL REFERENCES free_play_games(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL CHECK (sequence_number BETWEEN 1 AND 42),
    slot            SMALLINT NOT NULL CHECK (slot IN (1, 2)),
    player_id       UUID REFERENCES users(id),                    -- NULL = bot
    column_index    SMALLINT NOT NULL CHECK (column_index BETWEEN 0 AND 6),
    row_index       SMALLINT NOT NULL CHECK (row_index BETWEEN 0 AND 5),
    client_nonce    TEXT CHECK (client_nonce IS NULL OR char_length(client_nonce) BETWEEN 8 AND 64),
    state_hash      TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fp_moves_unique_sequence UNIQUE (game_id, sequence_number)
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_fp_moves_nonce
    ON free_play_moves (game_id, player_id, client_nonce) WHERE client_nonce IS NOT NULL;
