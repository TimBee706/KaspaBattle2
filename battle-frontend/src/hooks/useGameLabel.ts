import { useTranslation } from 'react-i18next';
import { getMatchProvider, type BattleMatch } from '../api/types';
import { SUPPORTED_GAMES } from '../config/constants';

/** Human readable game name of a match (FACEIT catalog name or the native game's name). */
export function useGameLabel(match: Pick<BattleMatch, 'provider' | 'game_id'>): string {
    const { t } = useTranslation();
    if (getMatchProvider(match) === 'NATIVE') {
        return match.game_id === 'connect-four' ? t('native.games.connect_four') : match.game_id;
    }
    return SUPPORTED_GAMES.find((g) => g.id === match.game_id)?.name ?? (match.game_id || 'CS2');
}
