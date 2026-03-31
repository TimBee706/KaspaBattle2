import apiClient from './client';
import type {
    BattleMatch, CreateMatchRequest, CreateMatchResponse,
    MatchListResponse, SubmitDepositRequest, AcceptMatchRequest,
    InitiateDisputeRequest, LobbyFilters, PaymentStatus,
    SubmitFaceitMatchIdRequest, SubmitFaceitMatchIdResponse,
} from './types';

export async function createMatch(data: CreateMatchRequest): Promise<CreateMatchResponse> {
    const res = await apiClient.post<any>('/challenges', {
        game_id: data.game_id,
        wager_sompi: data.wager_amount_sompi,
        mode: data.match_mode.toUpperCase()
    });
    return {
        match: res.data as BattleMatch,
        escrow_address: "mock" // Will be provided by escrow contract later
    };
}

export async function acceptMatch(data: AcceptMatchRequest): Promise<BattleMatch> {
    const res = await apiClient.post<BattleMatch>(`/matches/${data.match_id}/accept`);
    return res.data;
}

export async function getMatch(matchId: string): Promise<BattleMatch> {
    const res = await apiClient.get<BattleMatch>(`/matches/${matchId}`);
    return res.data;
}

export async function getOpenMatches(filters: LobbyFilters = {}): Promise<MatchListResponse> {
    const res = await apiClient.get<BattleMatch[]>('/lobbies');
    const openMatches = res.data.filter(m => m.status === 'OPEN');
    return { matches: openMatches, total: openMatches.length, page: 1, per_page: 20 };
}

export async function submitDeposit(data: SubmitDepositRequest): Promise<BattleMatch> {
    const res = await apiClient.post<BattleMatch>(`/matches/${data.match_id}/deposit`, {
        tx_hash: data.tx_hash,
        player_role: data.player_role,
    });
    return res.data;
}

export async function submitFaceitMatchId(
    data: SubmitFaceitMatchIdRequest,
): Promise<SubmitFaceitMatchIdResponse> {
    const res = await apiClient.post<SubmitFaceitMatchIdResponse>(
        `/matches/${data.match_id}/faceit-match-id`,
        { faceit_match_id: data.faceit_match_id },
    );
    return res.data;
}

export async function getMyMatches(page = 1, perPage = 20): Promise<MatchListResponse> {
    const res = await apiClient.get<MatchListResponse>('/matches/me', { params: { page, per_page: perPage } });
    return res.data;
}

export async function initiateDispute(data: InitiateDisputeRequest): Promise<BattleMatch> {
    const res = await apiClient.post<BattleMatch>(`/matches/${data.match_id}/dispute`, { reason: data.reason });
    return res.data;
}

/** Fetch per-player on-chain deposit confirmation status from the backend. */
export async function getPaymentStatus(matchId: string): Promise<PaymentStatus> {
    const res = await apiClient.get<PaymentStatus>(`/matches/${matchId}/payment-status`);
    return res.data;
}
