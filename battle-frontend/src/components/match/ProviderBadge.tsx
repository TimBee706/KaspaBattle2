import { useTranslation } from 'react-i18next';
import { getMatchProvider, type BattleMatch } from '../../api/types';
import { useGameLabel } from '../../hooks/useGameLabel';

/** "BROWSER GAME · VIER GEWINNT" or "FACEIT · <GAME>" — makes the provider unmistakable on a card. */
export function ProviderBadge({ match }: { match: Pick<BattleMatch, 'provider' | 'game_id'> }) {
    const { t } = useTranslation();
    const game = useGameLabel(match).toLocaleUpperCase();
    const isNative = getMatchProvider(match) === 'NATIVE';
    return (
        <span
            data-provider={isNative ? 'NATIVE' : 'FACEIT'}
            className={[
                'inline-flex items-center rounded-full border px-2.5 py-0.5 text-2xs font-black uppercase tracking-widest',
                isNative
                    ? 'border-kaspa-primary/40 bg-kaspa-primary/10 text-kaspa-primary'
                    : 'border-[#FF5500]/30 bg-[#FF5500]/10 text-[#FF5500]',
            ].join(' ')}
        >
            {isNative ? t('lobby.badge.native', { game }) : t('lobby.badge.faceit', { game })}
        </span>
    );
}
