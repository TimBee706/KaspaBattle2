import { useTranslation } from 'react-i18next';
import { KaspaCoin } from './ConnectFourCell';
import type { NativePlayer } from '../../domain/nativeGame';

interface Props {
    player: NativePlayer;
    isYou: boolean;
    isToMove: boolean;
    isWinner: boolean;
}

export function PlayerPanel({ player, isYou, isToMove, isWinner }: Props) {
    const { t } = useTranslation();
    const name = player.displayName || t('native.player.unknown');
    return (
        <div
            data-testid={`player-panel-${player.slot}`}
            aria-current={isToMove ? 'true' : undefined}
            className={[
                'flex items-center gap-3 rounded-xl border p-3 transition-all',
                isToMove ? 'border-kaspa-primary bg-kaspa-primary/10 shadow-glow-subtle' : 'border-kaspa-border bg-kaspa-dark/50',
                isWinner ? 'border-emerald-500/60 bg-emerald-900/20' : '',
            ].join(' ')}
        >
            <KaspaCoin color={player.color} className="h-9 w-9 shrink-0" />
            <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-bold text-white">
                    {name}
                    {isYou && <span className="ml-2 text-2xs font-black uppercase tracking-widest text-kaspa-primary">{t('native.player.you')}</span>}
                </p>
                <p className="text-2xs uppercase tracking-widest text-gray-500">
                    {player.color === 'blue' ? t('native.player.blue') : t('native.player.red')}
                    {' · '}
                    {player.slot === 1 ? t('native.player.first') : t('native.player.second')}
                </p>
            </div>
            {isWinner && <span className="text-2xs font-black uppercase tracking-widest text-emerald-400">{t('native.player.winner')}</span>}
            {isToMove && !isWinner && <span className="text-2xs font-black uppercase tracking-widest text-kaspa-primary">{t('native.player.to_move')}</span>}
        </div>
    );
}
