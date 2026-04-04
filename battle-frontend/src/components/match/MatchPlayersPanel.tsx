import { useTranslation } from 'react-i18next';
import type { BattleMatch, MatchPlayerInfo } from '../../api/types';
import { useAuthStore } from '../../stores/useAuthStore';

// ── Helpers ──────────────────────────────────────────────────────────────────

const FALLBACK_AVATAR = 'https://www.faceit.com/static/images/avatar.png';

const FACEIT_ICON = (
    <svg viewBox="0 0 24 24" fill="currentColor" className="w-3.5 h-3.5">
        <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" strokeWidth="1.5"
            fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
);

// ── Sub-component: Single Player Card ────────────────────────────────────────

interface PlayerCardProps {
    player: MatchPlayerInfo;
    label: string;
    isCurrentUser: boolean;
    accentColor: string;
    animation?: string;
}

function PlayerCard({ player, label, isCurrentUser, accentColor, animation }: PlayerCardProps) {
    const { t } = useTranslation();

    const handleFaceitClick = () => {
        if (player.faceitProfileUrl) {
            window.open(player.faceitProfileUrl, '_blank', 'noopener,noreferrer');
        }
    };

    return (
        <div
            className={`
                relative flex flex-col items-center gap-3 p-5 rounded-2xl
                border transition-all duration-300 group
                bg-gradient-to-b from-kaspa-card to-kaspa-dark
                ${accentColor}
                hover:shadow-lg hover:shadow-kaspa-primary/10
                hover:-translate-y-0.5
                ${animation ?? ''}
            `}
        >
            {/* Role badge */}
            <div className={`
                absolute top-3 left-3 px-2 py-0.5 rounded-full text-[9px]
                font-black uppercase tracking-wider
                ${isCurrentUser
                    ? 'bg-kaspa-primary/20 text-kaspa-primary border border-kaspa-primary/30'
                    : 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
                }
            `}>
                {isCurrentUser ? t('match.player_you') : label}
            </div>

            {/* Avatar */}
            <div className={`
                relative w-16 h-16 rounded-full overflow-hidden mt-4
                border-2 ring-2 ring-offset-1 ring-offset-kaspa-dark
                transition-all duration-300 group-hover:scale-105
                ${isCurrentUser
                    ? 'border-kaspa-primary/60 ring-kaspa-primary/20'
                    : 'border-blue-400/60 ring-blue-400/20'
                }
            `}>
                <img
                    src={player.avatarUrl ?? FALLBACK_AVATAR}
                    alt={player.nickname}
                    className="w-full h-full object-cover"
                    onError={(e) => {
                        (e.target as HTMLImageElement).src = FALLBACK_AVATAR;
                    }}
                />
                {/* FACEIT online indicator glow */}
                <div className={`
                    absolute bottom-0.5 right-0.5 w-3 h-3 rounded-full
                    border-2 border-kaspa-dark
                    ${isCurrentUser ? 'bg-kaspa-primary animate-pulse' : 'bg-blue-400'}
                `} />
            </div>

            {/* Nickname */}
            <div className="text-center">
                <span className={`
                    font-black text-sm tracking-tight block leading-tight
                    ${player.nickname === t('match.unknown_player')
                        ? 'text-gray-500 italic'
                        : 'text-white'
                    }
                `}>
                    {player.nickname}
                </span>
                {player.faceitId && (
                    <span className="text-[9px] text-gray-600 font-mono mt-0.5 block truncate max-w-[120px]">
                        ID: {player.faceitId.slice(0, 8)}...
                    </span>
                )}
            </div>

            {/* Action buttons */}
            <div className="flex gap-2 mt-1">
                {/* FACEIT Profile Link */}
                {player.faceitProfileUrl ? (
                    <button
                        id={`faceit-profile-${player.userId}`}
                        onClick={handleFaceitClick}
                        title={t('match.view_faceit_profile')}
                        className={`
                            flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[10px] font-bold
                            uppercase tracking-wide transition-all duration-200
                            bg-[#FF5500]/10 hover:bg-[#FF5500]/25 text-[#FF5500]
                            border border-[#FF5500]/20 hover:border-[#FF5500]/40
                            hover:scale-105 active:scale-95
                        `}
                    >
                        {FACEIT_ICON}
                        {t('match.view_faceit_profile')}
                    </button>
                ) : (
                    <div className="px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase tracking-wide
                        text-gray-600 border border-gray-800 bg-gray-900/50">
                        {t('match.no_faceit_link')}
                    </div>
                )}
            </div>

            {/* Deposit status indicator bar */}
            <div className={`
                w-full h-0.5 rounded-full mt-1 transition-all duration-700
                ${player.hasDeposited ? 'bg-kaspa-primary' : 'bg-gray-800'}
            `} />
        </div>
    );
}

// ── Empty Slot (waiting for opponent) ────────────────────────────────────────

function EmptyPlayerSlot() {
    const { t } = useTranslation();
    return (
        <div className="flex flex-col items-center justify-center gap-3 p-5 rounded-2xl
            border-2 border-dashed border-kaspa-border bg-kaspa-dark/40 min-h-[180px]">
            <div className="w-16 h-16 rounded-full bg-kaspa-dark border-2 border-dashed
                border-gray-700 flex items-center justify-center text-gray-600 text-2xl">
                ?
            </div>
            <span className="text-xs text-gray-600 font-bold uppercase tracking-widest animate-pulse">
                {t('lobby.waiting_for_opponent')}
            </span>
        </div>
    );
}

// ── Main Component: MatchPlayersPanel ─────────────────────────────────────────

interface MatchPlayersPanelProps {
    match: BattleMatch;
}

/** Extracts MatchPlayerInfo from match data for both players */
function buildPlayerInfo(match: BattleMatch): {
    playerA: MatchPlayerInfo;
    playerB: MatchPlayerInfo | null;
} {
    const unknownNickname = match.player_a_faceit_nickname || 'Unbekannter Spieler';

    const playerA: MatchPlayerInfo = {
        userId: match.creator_user_id ?? '',
        faceitId: match.player_a_faceit_id || null,
        nickname: match.player_a_faceit_nickname || unknownNickname,
        avatarUrl: match.player_a_avatar_url ?? null,
        faceitProfileUrl: match.player_a_faceit_profile_url ?? null,
        hasDeposited: !!match.player_a_deposit_tx_hash,
    };

    const playerB: MatchPlayerInfo | null = match.opponent_user_id
        ? {
            userId: match.opponent_user_id ?? '',
            faceitId: match.player_b_faceit_id || null,
            nickname: match.player_b_faceit_nickname || 'Unbekannter Spieler',
            avatarUrl: match.player_b_avatar_url ?? null,
            faceitProfileUrl: match.player_b_faceit_profile_url ?? null,
            hasDeposited: !!match.player_b_deposit_tx_hash,
        }
        : null;

    return { playerA, playerB };
}

export function MatchPlayersPanel({ match }: MatchPlayersPanelProps) {
    const { t } = useTranslation();
    const { user } = useAuthStore();
    const currentUserId = user?.id ?? null;

    const { playerA, playerB } = buildPlayerInfo(match);

    const isPlayerACurrentUser = currentUserId === match.creator_user_id;
    const isPlayerBCurrentUser = currentUserId === match.opponent_user_id;

    return (
        <div className="card p-0 overflow-hidden bg-gradient-to-br from-kaspa-card via-kaspa-dark to-kaspa-card">
            {/* Header strip */}
            <div className="px-6 pt-5 pb-3 border-b border-kaspa-border/50">
                <p className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em]">
                    {t('match.players')}
                </p>
            </div>

            <div className="p-5">
                <div className="grid grid-cols-1 sm:grid-cols-[1fr_auto_1fr] gap-4 items-center">
                    {/* Player A */}
                    <PlayerCard
                        player={playerA}
                        label={t('match.challenger')}
                        isCurrentUser={isPlayerACurrentUser}
                        accentColor="border-kaspa-primary/20"
                        animation="animate-in fade-in slide-in-from-left-4 duration-500"
                    />

                    {/* VS Divider */}
                    <div className="flex sm:flex-col items-center justify-center gap-2 py-2 sm:py-0">
                        <div className="hidden sm:block w-px h-8 bg-gradient-to-b from-transparent via-kaspa-border to-transparent" />
                        <span className="text-2xl font-black italic opacity-20 select-none">
                            VS
                        </span>
                        <div className="hidden sm:block w-px h-8 bg-gradient-to-b from-transparent via-kaspa-border to-transparent" />
                    </div>

                    {/* Player B or empty slot */}
                    {playerB ? (
                        <PlayerCard
                            player={playerB}
                            label={t('match.opponent')}
                            isCurrentUser={isPlayerBCurrentUser}
                            accentColor="border-blue-500/20"
                            animation="animate-in fade-in slide-in-from-right-4 duration-500"
                        />
                    ) : (
                        <EmptyPlayerSlot />
                    )}
                </div>
            </div>
        </div>
    );
}
