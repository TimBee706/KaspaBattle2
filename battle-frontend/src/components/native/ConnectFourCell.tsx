import { memo } from 'react';

export type CellValue = 0 | 1 | 2;

interface CoinProps {
    color: 'blue' | 'red';
    className?: string;
}

/** Kaspa coin: gradient disc, inner ring and a bold "K" style glyph. Purely decorative. */
export function KaspaCoin({ color, className = '' }: CoinProps) {
    const id = `coin-${color}`;
    const [from, to, ring] = color === 'blue'
        ? ['#60a5fa', '#1d4ed8', '#bfdbfe']
        : ['#f87171', '#b91c1c', '#fecaca'];
    return (
        <svg viewBox="0 0 48 48" className={className} aria-hidden="true" focusable="false">
            <defs>
                <radialGradient id={id} cx="35%" cy="30%" r="80%">
                    <stop offset="0%" stopColor={from} />
                    <stop offset="100%" stopColor={to} />
                </radialGradient>
            </defs>
            <circle cx="24" cy="24" r="22" fill={`url(#${id})`} />
            <circle cx="24" cy="24" r="16.5" fill="none" stroke={ring} strokeOpacity="0.55" strokeWidth="1.5" />
            <path
                d="M18 14v20M18 24l10-10M18 24l10 10"
                fill="none"
                stroke={ring}
                strokeOpacity="0.9"
                strokeWidth="3.2"
                strokeLinecap="round"
                strokeLinejoin="round"
            />
        </svg>
    );
}

interface Props {
    value: CellValue;
    /** part of the winning four */
    isWinning?: boolean;
    /** most recently dropped disc (gets the drop animation) */
    isLast?: boolean;
    /** translucent preview of the disc that would land here */
    previewColor?: 'blue' | 'red' | null;
}

/** One board slot. Not interactive itself — the whole column is the button. */
export const ConnectFourCell = memo(function ConnectFourCell({ value, isWinning, isLast, previewColor }: Props) {
    const color = value === 1 ? 'blue' : value === 2 ? 'red' : null;
    return (
        <span
            className={[
                'relative block aspect-square w-full rounded-full',
                'bg-kaspa-dark border border-kaspa-border shadow-inner',
                isWinning ? 'ring-2 ring-kaspa-primary shadow-glow-primary' : '',
            ].join(' ')}
            data-cell-value={value}
            data-winning={isWinning ? 'true' : undefined}
        >
            {color && (
                <KaspaCoin
                    color={color}
                    className={`absolute inset-[6%] h-[88%] w-[88%] ${isLast ? 'c4-coin-drop' : ''} ${isWinning ? 'c4-coin-win' : ''}`}
                />
            )}
            {!color && previewColor && (
                <KaspaCoin color={previewColor} className="absolute inset-[6%] h-[88%] w-[88%] opacity-30" />
            )}
        </span>
    );
});
