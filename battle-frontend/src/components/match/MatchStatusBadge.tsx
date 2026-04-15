import type { MatchStatus } from '../../api/types';
import { useTranslation } from 'react-i18next';

const STATUS_CONFIG: Record<MatchStatus, { key: string; color: string; pulse?: boolean }> = {
    DRAFT: { key: 'status.DRAFT', color: 'bg-gray-500' },
    OPEN: { key: 'status.OPEN', color: 'bg-blue-600' },
    AWAITING_FUNDING: { key: 'status.AWAITING_FUNDING', color: 'bg-yellow-600', pulse: true },
    FUNDED: { key: 'status.FUNDED', color: 'bg-indigo-600', pulse: true },
    LOCKED: { key: 'status.LOCKED', color: 'bg-orange-600', pulse: true },
    GAME_ID_INPUT: { key: 'status.GAME_ID_INPUT', color: 'bg-cyan-600', pulse: true },
    IN_GAME: { key: 'status.IN_GAME', color: 'bg-purple-600', pulse: true },
    FINISHED_FACEIT: { key: 'status.FINISHED_FACEIT', color: 'bg-teal-600' },
    READY_FOR_PAYOUT: { key: 'status.READY_FOR_PAYOUT', color: 'bg-emerald-700', pulse: true },
    RESOLVING: { key: 'status.RESOLVING', color: 'bg-amber-600', pulse: true },
    RESOLVED: { key: 'status.RESOLVED', color: 'bg-green-600' },
    PAID_OUT: { key: 'status.PAID_OUT', color: 'bg-emerald-600' },
    DISPUTED: { key: 'status.DISPUTED', color: 'bg-red-600', pulse: true },
    CANCELLED: { key: 'status.CANCELLED', color: 'bg-gray-600' },
    REFUNDED: { key: 'status.REFUNDED', color: 'bg-emerald-600' },
};

const FALLBACK_STATUS_CONFIG = { key: 'status.UNKNOWN', color: 'bg-slate-600' } as const;

export function MatchStatusBadge({ status }: { status: MatchStatus | string }) {
    const config = STATUS_CONFIG[status as MatchStatus] ?? FALLBACK_STATUS_CONFIG;
    const { t } = useTranslation();

    return (
        <span className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-[10px] font-black uppercase tracking-widest text-white shadow-sm shadow-black/20 ${config.color}`}>
            {config.pulse && (
                <span className="relative flex h-2 w-2">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-white"></span>
                </span>
            )}
            {config.key === 'status.UNKNOWN' ? status : t(config.key)}
        </span>
    );
}
