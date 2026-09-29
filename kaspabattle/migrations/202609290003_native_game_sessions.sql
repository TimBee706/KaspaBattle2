-- Native Games (Phase 1c): server-authoritative game sessions + move audit log.

CREATE TABLE IF NOT EXISTS native_game_sessions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id          UUID NOT NULL UNIQUE REFERENCES matches(id) ON DELETE CASCADE,
    game_type         TEXT NOT NULL CHECK (game_type IN ('CONNECT_FOUR')),
    status            TEXT NOT NULL DEFAULT 'READY' CHECK (status IN ('READY', 'ACTIVE', 'FINISHED')),
    player_one_id     UUID NOT NULL REFERENCES users(id),   -- match creator, moves first
    player_two_id     UUID NOT NULL REFERENCES users(id),   -- match opponent
    board_state       JSONB NOT NULL,
    current_player_id UUID REFERENCES users(id),
    winner_user_id    UUID REFERENCES users(id),
    result            TEXT CHECK (result IN ('WIN', 'DRAW')),
    end_reason        TEXT CHECK (end_reason IN ('CONNECT_FOUR', 'DRAW', 'RESIGNATION', 'TIMEOUT')),
    move_count        INTEGER NOT NULL DEFAULT 0 CHECK (move_count BETWEEN 0 AND 42),
    version           BIGINT  NOT NULL DEFAULT 0,
    state_hash        TEXT    NOT NULL,
    last_move_column  SMALLINT CHECK (last_move_column BETWEEN 0 AND 6),
    last_move_row     SMALLINT CHECK (last_move_row BETWEEN 0 AND 5),
    turn_deadline     TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at        TIMESTAMPTZ,
    finished_at       TIMESTAMPTZ,
    CONSTRAINT native_sessions_distinct_players CHECK (player_one_id <> player_two_id),
    CONSTRAINT native_sessions_finished_consistency CHECK (
        (status = 'FINISHED') = (result IS NOT NULL AND end_reason IS NOT NULL AND finished_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_native_sessions_active_deadline
    ON native_game_sessions (turn_deadline) WHERE status = 'ACTIVE';

CREATE TABLE IF NOT EXISTS native_game_moves (
    id              BIGSERIAL PRIMARY KEY,
    match_id        UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    sequence_number INTEGER  NOT NULL CHECK (sequence_number >= 1),
    player_id       UUID     NOT NULL REFERENCES users(id),
    column_index    SMALLINT NOT NULL CHECK (column_index BETWEEN 0 AND 6),
    row_index       SMALLINT NOT NULL CHECK (row_index BETWEEN 0 AND 5),
    client_nonce    TEXT     NOT NULL CHECK (char_length(client_nonce) BETWEEN 8 AND 64),
    state_hash      TEXT     NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT native_moves_unique_sequence UNIQUE (match_id, sequence_number),
    CONSTRAINT native_moves_unique_nonce    UNIQUE (match_id, player_id, client_nonce)
);
