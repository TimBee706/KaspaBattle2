import { useTranslation } from 'react-i18next';
import type { BattleMatch } from '../../api/types';
import { useConnectFourGame } from '../../hooks/useConnectFourGame';
import { useMatchStore } from '../../stores/useMatchStore';
import { getMatch } from '../../api/matches';
import { ConnectFourBoard } from './ConnectFourBoard';
import { GameStatusPanel } from './GameStatusPanel';
import { PlayerPanel } from './PlayerPanel';
import { hasGameSession } from '../../domain/nativeGame';

/**
 * Renders the browser game of a NATIVE match: two player panels, the board and the status panel.
 * Everything shown comes from the server snapshot (`useConnectFourGame`).
 */
export function NativeGameContainer({ match }: { match: BattleMatch }) {
    const { t } = useTranslation();
    const setMatch = useMatchStore((s) => s.setMatch);
    const enabled = hasGameSession(match);
    const game = useConnectFourGame(enabled ? match.id : null, {
        // A match_update means lifecycle changes (payout, refund…): refresh the match too.
        onMatchUpdate: () => { void getMatch(match.id).then(setMatch).catch(() => undefined); },
    });
    const { snapshot } = game;

    if (!enabled || game.notStarted) {
        return (
            <div className="glass-panel p-6 text-center" data-testid="native-waiting">
                <p className="text-sm font-bold text-gray-400">{t('native.waiting_for_funding')}</p>
            </div>
        );
    }

    if (!snapshot) {
        return (
            <div className="glass-panel flex items-center justify-center p-10" data-testid="native-loading">
                {game.error
                    ? <p role="alert" className="text-sm font-bold text-red-300">{t('native.error.generic')}</p>
                    : <div className="h-10 w-10 animate-spin rounded-full border-4 border-kaspa-primary border-t-transparent" aria-label={t('common.loading')} />}
            </div>
        );
    }

    const myColor = game.mySlot === 1 ? 'blue' : game.mySlot === 2 ? 'red' : null;
    const userId = snapshot.you?.userId ?? null;

    return (
        <div className="glass-panel space-y-4 p-4 sm:p-6" data-testid="native-game">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                {snapshot.players.map((player) => (
                    <PlayerPanel
                        key={player.userId}
                        player={player}
                        isYou={game.mySlot === player.slot || player.userId === userId}
                        isToMove={snapshot.status === 'ACTIVE' && snapshot.currentPlayerId === player.userId}
                        isWinner={snapshot.winnerUserId === player.userId}
                    />
                ))}
            </div>

            <ConnectFourBoard
                snapshot={snapshot}
                myColor={myColor}
                canPlay={game.myTurn && !game.isSubmitting}
                onDrop={(column) => void game.dropDisc(column)}
            />

            <GameStatusPanel
                snapshot={snapshot}
                mySlot={game.mySlot}
                myTurn={game.myTurn}
                secondsLeft={game.secondsLeft}
                connection={game.connection}
                isSubmitting={game.isSubmitting}
                errorCode={game.error}
                onResign={() => void game.resign()}
            />
        </div>
    );
}
