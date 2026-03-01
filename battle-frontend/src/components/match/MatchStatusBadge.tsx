import type { MatchStatus } from '../../api/types';
import { useTranslation } from 'react-i18next';

const STATUS_CONFIG: Record<MatchStatus, { key: string; color: string; pulse?: boolean }> = {
    OPEN: { key: 'status.OPEN', color: 'bg-blue-600' },
    FUNDED: { key: 'status.FUNDED', color: 'bg-indigo-600', pulse: true },
    LOCKED: { key: 'status.LOCKED', color: 'bg-orange-600', pulse: true },
    RESOLVED: { key: 'status.RESOLVED', color: 'bg-green-600' },
    PAID_OUT: { key: 'status.PAID_OUT', color: 'bg-emerald-600' },
    DISPUTED: { key: 'status.DISPUTED', color: 'bg-red-600', pulse: true },
    CANCELLED: { key: 'status.CANCELLED', color: 'bg-gray-600' },
};

export function MatchStatusBadge({ status }: { status: MatchStatus }) {
    const config = STATUS_CONFIG[status];
    const { t } = useTranslation();

    return (
        <span className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-[10px] font-black uppercase tracking-widest text-white shadow-sm shadow-black/20 ${config.color}`}>
            {config.pulse && (
                <span className="relative flex h-2 w-2">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-white"></span>
                </span>
            )}
            {t(config.key)}
        </span>
    );
}
