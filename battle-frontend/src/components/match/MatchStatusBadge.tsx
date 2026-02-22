import type { MatchStatus } from '../../api/types';

const STATUS_CONFIG: Record<MatchStatus, { label: string; color: string; pulse?: boolean }> = {
    OPEN: { label: 'Offen', color: 'bg-blue-600' },
    FUNDED: { label: 'Gefundinged', color: 'bg-indigo-600', pulse: true },
    LOCKED: { label: 'Im Spiel', color: 'bg-orange-600', pulse: true },
    RESOLVED: { label: 'Beendet', color: 'bg-green-600' },
    PAID_OUT: { label: 'Ausgezahlt', color: 'bg-emerald-600' },
    DISPUTED: { label: 'Dispute', color: 'bg-red-600', pulse: true },
    CANCELLED: { label: 'Abgebrochen', color: 'bg-gray-600' },
};

export function MatchStatusBadge({ status }: { status: MatchStatus }) {
    const config = STATUS_CONFIG[status];

    return (
        <span className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-[10px] font-black uppercase tracking-widest text-white shadow-sm shadow-black/20 ${config.color}`}>
            {config.pulse && (
                <span className="relative flex h-2 w-2">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-white"></span>
                </span>
            )}
            {config.label}
        </span>
    );
}
