export type MatchMode = 'BO1' | 'BO3';
export type MatchStatus = 'DRAFT' | 'OPEN' | 'AWAITING_FUNDING' | 'FUNDED' | 'LOCKED' | 'IN_GAME' | 'RESOLVING' | 'RESOLVED' | 'PAID_OUT' | 'DISPUTED' | 'CANCELLED';

export interface BattleMatch {
    id: string;                            // UUID
    creator_user_id?: string;              // UUID
    opponent_user_id?: string | null;      // UUID
    player_a_kas_address: string;
    player_b_kas_address: string | null;
    player_a_faceit_id: string;
    player_b_faceit_id: string | null;
    player_a_faceit_nickname: string;
    player_b_faceit_nickname: string | null;
    faceit_match_id: string | null;
    wager_amount_sompi: number;
    stake_kas?: number;                   // Backend property fallback
    escrow_address: string;
    status: MatchStatus;
    game_id: string;                       // 'cs2' | 'dota2' | 'valorant'
    match_mode: string;                    // 'bo1' | 'bo3'
    mode?: string;                         // Backend property fallback
    winner_kas_address: string | null;
    winner_faceit_nickname: string | null;
    payout_tx_hash: string | null;
    score: string | null;                  // z.B. '16:7'
    player_a_deposit_tx_hash: string | null;
    player_b_deposit_tx_hash: string | null;
    created_at: string;                    // ISO 8601
    locked_at: string | null;
    resolved_at: string | null;
    timeout_at: string;
}

// ── Request Types ──
export interface CreateMatchRequest {
    game_id: string;
    match_mode: string;
    wager_amount_sompi: number;
}

export interface AcceptMatchRequest {
    match_id: string;
}

export interface SubmitDepositRequest {
    match_id: string;
    tx_hash: string;
    player_role: 'A' | 'B';
}

export interface InitiateDisputeRequest {
    match_id: string;
    reason: string;
}

// ── Response Types ──
export interface CreateMatchResponse {
    match: BattleMatch;
    escrow_address: string;
}

export interface MatchListResponse {
    matches: BattleMatch[];
    total: number;
    page: number;
    per_page: number;
}

// ── User Types ──
export interface UserProfile {
    id: string;
    faceit_id: string;
    faceit_nickname: string;
    display_name?: string;               // Backend property / TestUser
    faceit_avatar: string;
    kaspa_address: string;
    total_matches: number;
    wins: number;
    losses: number;
    total_wagered_sompi: number;
    total_won_sompi: number;
    created_at: string;
}

// ── Auth Types ──
export interface AuthTokens {
    access_token: string;
    refresh_token: string;
    expires_at: number;
}

export interface FaceitAuthResponse {
    tokens: AuthTokens;
    user: UserProfile;
}

// ── Faceit Data API Types ──
export interface FaceitProfileResponse {
    faceit_player_id: string;
    nickname: string;
    avatar_url: string | null;
    game_id: string;
    elo: number;
    skill_level: number;
    is_cached: boolean;
}

export interface FaceitMatchDetails {
    winner: string;
    score: Record<string, number>;
}

export interface FaceitMatchHistoryItem {
    match_id: string;
    game_id: string;
    region: string;
    match_type: string;
    game_mode: string;
    map_i_ds: string[];
    team_id: string;
    playing_players: string[];
    started_at: number;
    finished_at: number;
    results: FaceitMatchDetails;
}

export interface FaceitMatchHistoryResponse {
    items: FaceitMatchHistoryItem[];
    start: number;
    end: number;
}


// ── Filter Types ──
export interface LobbyFilters {
    game_id?: string;
    min_wager_kas?: number;
    max_wager_kas?: number;
    match_mode?: string;
    status?: MatchStatus;
    page?: number;
    per_page?: number;
}
