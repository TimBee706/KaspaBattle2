import apiClient from './client';

// ─── Types ────────────────────────────────────────────────────────────────────

export interface Tournament {
  id: string;
  name: string;
  game_id: string;
  status: TournamentStatus;
  max_teams: number;
  buy_in_sompi: number;
  prize_winner_pct: number;
  prize_runner_up_pct: number;
  platform_fee_pct: number;
  total_prize_pool_sompi: number;
  registration_deadline: string | null;
  bracket_locked_at: string | null;
  escrow_address: string | null;
  payout_tx_hash: string | null;
  payout_executed_at: string | null;
  winner_team_id_ref: string | null;
  runner_up_team_id_ref: string | null;
  created_at: string;
  organizer_user_id: string;
}

export type TournamentStatus =
  | 'DRAFT'
  | 'REGISTRATION'
  | 'FUNDED'
  | 'BRACKET_READY'
  | 'IN_PROGRESS'
  | 'COMPLETED'
  | 'CANCELLED'
  | 'DISPUTED';

export interface TournamentTeam {
  id: string;
  tournament_id: string;
  name: string;
  seed: number | null;
  captain_user_id: string;
  captain_display_name?: string;
  deposit_status: 'PENDING' | 'CONFIRMED';
  deposit_tx_hash: string | null;
  members?: TeamMember[];
}

export interface TeamMember {
  user_id: string;
  display_name: string;
  faceit_nickname: string | null;
  faceit_elo: number | null;
}

export interface BracketSlot {
  id: string;
  round: number;
  slot_index: number;
  status: 'WAITING' | 'READY' | 'IN_PROGRESS' | 'COMPLETED';
  team_a: { id: string; name: string; seed: number | null } | null;
  team_b: { id: string; name: string; seed: number | null } | null;
  winner_team_id: string | null;
  faceit_match_id: string | null;
  reported_score?: string;
  match_started_at: string | null;
  match_finished_at: string | null;
  disputed?: boolean;
}

export interface TournamentResults {
  tournament_id: string;
  status: string;
  winner_team: { id: string; name: string; captain_display_name: string | null } | null;
  runner_up_team: { id: string; name: string; captain_display_name: string | null } | null;
  payout_tx_hash: string | null;
  payout_executed_at: string | null;
  total_prize_pool_sompi: number;
  winner_share_sompi: number;
  runner_up_share_sompi: number;
  platform_fee_sompi: number;
  bracket: BracketSlot[];
}

export interface PayoutInfo {
  tournament_id: string;
  escrow_address: string | null;
  total_prize_pool_sompi: number;
  winner_share_sompi: number;
  runner_up_share_sompi: number;
  platform_fee_sompi: number;
  winner_team: { id: string; name: string } | null;
  winner_kaspa_address: string | null;
  payout_tx_hash: string | null;
  payout_executed_at: string | null;
  status: string;
}

// ─── Tournament CRUD ──────────────────────────────────────────────────────────

export const listTournaments = async (): Promise<Tournament[]> => {
  const { data } = await apiClient.get('/tournaments');
  return data;
};

export const getTournament = async (id: string): Promise<Tournament> => {
  const { data } = await apiClient.get(`/tournaments/${id}`);
  return data;
};

export const createTournament = async (payload: {
  name: string;
  game_id: string;
  max_teams: number;
  buy_in_sompi: number;
  prize_winner_pct: number;
  prize_runner_up_pct: number;
  platform_fee_pct: number;
  registration_deadline?: string;
}): Promise<Tournament> => {
  const { data } = await apiClient.post('/tournaments', payload);
  return data;
};

// ─── Teams ───────────────────────────────────────────────────────────────────

export const listTeams = async (tournamentId: string): Promise<TournamentTeam[]> => {
  const { data } = await apiClient.get(`/tournaments/${tournamentId}/teams`);
  return data;
};

export const registerTeam = async (tournamentId: string, teamName: string): Promise<TournamentTeam> => {
  const { data } = await apiClient.post(`/tournaments/${tournamentId}/teams`, { name: teamName });
  return data;
};

export const addTeamMember = async (
  tournamentId: string,
  teamId: string,
  faceitNickname: string
): Promise<void> => {
  await apiClient.post(`/tournaments/${tournamentId}/teams/${teamId}/members`, {
    faceit_nickname: faceitNickname,
  });
};

// ─── Bracket ─────────────────────────────────────────────────────────────────

export const lockBracket = async (tournamentId: string): Promise<void> => {
  await apiClient.post(`/tournaments/${tournamentId}/lock`);
};

export const getBracket = async (tournamentId: string): Promise<BracketSlot[]> => {
  const { data } = await apiClient.get(`/tournaments/${tournamentId}/bracket`);
  return data;
};

export const submitBracketMatchId = async (
  tournamentId: string,
  slotId: string,
  faceitMatchId: string
): Promise<void> => {
  await apiClient.post(`/tournaments/${tournamentId}/bracket/${slotId}/match-id`, {
    faceit_match_id: faceitMatchId,
  });
};

// ─── Disputes ────────────────────────────────────────────────────────────────

export const fileTournamentDispute = async (tournamentId: string, reason: string): Promise<void> => {
  await apiClient.post(`/tournaments/${tournamentId}/dispute`, { reason });
};

export const fileBracketDispute = async (
  tournamentId: string,
  slotId: string,
  reason: string
): Promise<void> => {
  await apiClient.post(`/tournaments/${tournamentId}/bracket/${slotId}/dispute`, { reason });
};

// ─── Results & Payout ────────────────────────────────────────────────────────

export const getTournamentResults = async (tournamentId: string): Promise<TournamentResults> => {
  const { data } = await apiClient.get(`/tournaments/${tournamentId}/results`);
  return data;
};

export const getPayoutInfo = async (tournamentId: string): Promise<PayoutInfo> => {
  const { data } = await apiClient.get(`/tournaments/${tournamentId}/payout`);
  return data;
};

export const cancelTournament = async (tournamentId: string): Promise<void> => {
  await apiClient.post(`/tournaments/${tournamentId}/cancel`);
};

// ─── Admin ───────────────────────────────────────────────────────────────────

export const adminResolveDispute = async (
  tournamentId: string,
  winnerTeamId: string
): Promise<void> => {
  await apiClient.post(`/admin/tournaments/${tournamentId}/resolve-dispute`, {
    winner_team_id: winnerTeamId,
  });
};

export const adminResolveBracketDispute = async (
  tournamentId: string,
  slotId: string,
  winnerTeamId: string
): Promise<void> => {
  await apiClient.post(
    `/admin/tournaments/${tournamentId}/bracket/${slotId}/resolve-dispute`,
    { winner_team_id: winnerTeamId }
  );
};

export const adminTriggerPayout = async (tournamentId: string): Promise<void> => {
  await apiClient.post(`/admin/tournaments/${tournamentId}/payout`);
};

export const adminCancelTournament = async (tournamentId: string): Promise<void> => {
  await apiClient.post(`/admin/tournaments/${tournamentId}/cancel`);
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

export const sompiToKas = (sompi: number): string => {
  return (sompi / 1e8).toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  });
};

export const STATUS_LABELS: Record<TournamentStatus, string> = {
  DRAFT: 'Draft',
  REGISTRATION: 'Registration Open',
  FUNDED: 'Funded',
  BRACKET_READY: 'Bracket Ready',
  IN_PROGRESS: 'In Progress',
  COMPLETED: 'Completed',
  CANCELLED: 'Cancelled',
  DISPUTED: 'Disputed',
};

export const STATUS_COLORS: Record<TournamentStatus, string> = {
  DRAFT: '#6b7280',
  REGISTRATION: '#3b82f6',
  FUNDED: '#8b5cf6',
  BRACKET_READY: '#f59e0b',
  IN_PROGRESS: '#10b981',
  COMPLETED: '#49EACB',
  CANCELLED: '#ef4444',
  DISPUTED: '#f97316',
};
