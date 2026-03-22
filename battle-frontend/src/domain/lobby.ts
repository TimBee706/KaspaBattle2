import type { BattleMatch, PaymentStatus, PlayerPaymentInfo } from '../api/types';

export type LobbyRole = 'creator' | 'opponent' | 'viewer';
export type PlayerRole = 'A' | 'B';

export interface IdentityScopedDeposit {
    txHash: string;
    matchId: string;
    userId: string | null;
    walletAddress: string | null;
}

export function getLobbyRole(
    lobby: Pick<BattleMatch, 'creator_user_id' | 'opponent_user_id'>,
    currentUserId?: string | null,
): LobbyRole {
    if (!currentUserId) return 'viewer';
    if (lobby.creator_user_id === currentUserId) return 'creator';
    if (lobby.opponent_user_id === currentUserId) return 'opponent';
    return 'viewer';
}

export function isMyLobby(
    lobby: Pick<BattleMatch, 'creator_user_id' | 'opponent_user_id'>,
    currentUserId?: string | null,
): boolean {
    return getLobbyRole(lobby, currentUserId) !== 'viewer';
}

export function isAvailableChallenge(
    lobby: Pick<BattleMatch, 'status' | 'creator_user_id' | 'opponent_user_id'>,
    currentUserId?: string | null,
): boolean {
    return lobby.status === 'OPEN'
        && !lobby.opponent_user_id
        && getLobbyRole(lobby, currentUserId) === 'viewer';
}

export function getPlayerRoleForLobby(
    lobby: Pick<BattleMatch, 'creator_user_id' | 'opponent_user_id'>,
    currentUserId?: string | null,
): PlayerRole | null {
    const role = getLobbyRole(lobby, currentUserId);
    if (role === 'creator') return 'A';
    if (role === 'opponent') return 'B';
    return null;
}

export function needsPlayerDeposit(
    lobby: Pick<
        BattleMatch,
        'status' | 'creator_user_id' | 'opponent_user_id' | 'player_a_deposit_tx_hash' | 'player_b_deposit_tx_hash'
    >,
    currentUserId?: string | null,
): boolean {
    const playerRole = getPlayerRoleForLobby(lobby, currentUserId);
    if (!playerRole) return false;

    const isDepositPhase = lobby.status === 'OPEN' || lobby.status === 'AWAITING_FUNDING';
    if (!isDepositPhase) return false;

    if (playerRole === 'A') return !lobby.player_a_deposit_tx_hash;
    return !!lobby.opponent_user_id && !lobby.player_b_deposit_tx_hash;
}

export function getPaymentInfoForPlayer(
    paymentStatus: PaymentStatus | null,
    playerRole: PlayerRole | null,
): PlayerPaymentInfo | null {
    if (!paymentStatus || !playerRole) return null;
    return playerRole === 'A' ? paymentStatus.playerA : paymentStatus.playerB;
}

export function hasPlayerDeposited(
    paymentStatus: PaymentStatus | null,
    playerRole: PlayerRole | null,
): boolean {
    return getPaymentInfoForPlayer(paymentStatus, playerRole)?.paid ?? false;
}

export function isLocalDepositForIdentity(
    localDeposit: IdentityScopedDeposit | null,
    current: { matchId: string; userId?: string | null; walletAddress?: string | null },
): boolean {
    if (!localDeposit) return false;

    return localDeposit.matchId === current.matchId
        && localDeposit.userId === (current.userId ?? null)
        && localDeposit.walletAddress === (current.walletAddress ?? null);
}
