export type MatchMode = 'BO1' | 'BO3';
export type MatchStatus =
    | 'DRAFT'
    | 'OPEN'
    | 'AWAITING_FUNDING'
    | 'FUNDED'
    | 'LOCKED'
    | 'GAME_ID_INPUT'
    | 'IN_GAME'
    | 'FINISHED_FACEIT'
    | 'READY_FOR_PAYOUT'
    | 'RESOLVING'
    | 'RESOLVED'
    | 'PAID_OUT'
    | 'DISPUTED'
    | 'CANCELLED';

// ── Payment Status Types ──
export interface PlayerPaymentInfo {
    paid: boolean;
    confirmed_sompi: number;
    payment_count: number;
    min_confirmations: number;
}

export interface PaymentStatus {
    escrow_address: string;
    required_per_player_sompi: number;
    min_confirmations_required: number;
    playerA: PlayerPaymentInfo;
    playerB: PlayerPaymentInfo;
    both_paid: boolean;
}


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
    faceit_match_id_player_a?: string | null;
    faceit_match_id_player_b?: string | null;
    faceit_match_id_final?: string | null;
    faceit_match_status?: string | null;
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

export interface SubmitFaceitMatchIdRequest {
    match_id: string;
    faceit_match_id: string;
}

export interface SubmitFaceitMatchIdResponse {
    status: 'submitted' | 'confirmed';
    both_submitted: boolean;
    match_status: MatchStatus;
    faceit_match_id?: string;
    message?: string;
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
    faceit_connected: boolean;               // Backend-verifizierter FACEIT-Status
    display_name?: string;                   // Backend property / TestUser
    faceit_avatar: string;
    faceit_elo: number | null;               // Gecachte ELO aus faceit_links
    faceit_skill_level: number | null;       // FACEIT Level 1-10
    kaspa_address: string | null;
    total_matches: number;
    wins: number;
    losses: number;
    total_wagered_sompi: number;
    total_won_sompi: number;
    created_at: string;
}

export interface PlayerAccount {
    faceit: {
        userId: string;
        nickname: string;
        eloLevel: number | null;
        avatarUrl: string;
    } | null;
    wallet: {
        address: string;
        connectedAt: number | null;
    } | null;
    isFullyConnected: boolean;
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
    country: string;
    elo: number;
    skill_level: number;
    games: string[];
    faceit_url: string;
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
