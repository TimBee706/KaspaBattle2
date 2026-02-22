import apiClient from './client';
import type {
    BattleMatch, CreateMatchRequest, CreateMatchResponse,
    MatchListResponse, SubmitDepositRequest, AcceptMatchRequest,
    InitiateDisputeRequest, LobbyFilters,
} from './types';

export async function createMatch(data: CreateMatchRequest): Promise<CreateMatchResponse> {
    const res = await apiClient.post<CreateMatchResponse>('/matches', data);
    return res.data;
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
    const res = await apiClient.get<MatchListResponse>('/matches', { params: { ...filters, status: 'OPEN' } });
    return res.data;
}

export async function submitDeposit(data: SubmitDepositRequest): Promise<BattleMatch> {
    const res = await apiClient.post<BattleMatch>(`/matches/${data.match_id}/deposit`, {
        tx_hash: data.tx_hash,
        player_role: data.player_role,
    });
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
