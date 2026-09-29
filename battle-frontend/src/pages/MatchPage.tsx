import { useParams, useNavigate } from 'react-router-dom';
import { MatchDetailView } from '../components/match/MatchDetailView';
import { NativeMatchView } from '../components/native/NativeMatchView';
import { getMatchProvider } from '../api/types';
import { useMatchPolling } from '../hooks/useMatchPolling';
import { usePaymentStatus } from '../hooks/usePaymentStatus';
import { useMatchStore } from '../stores/useMatchStore';
import { useAuthStore } from '../stores/useAuthStore';
import { useTranslation } from 'react-i18next';
import { formatKas } from '../utils/format';
import { getPaymentInfoForPlayer, getPlayerRoleForLobby } from '../domain/lobby';
import { Icon } from '../components/Icon';

// Compact deposit status badge shown inline on the match page during AWAITING_FUNDING
function DepositStatusBanner({ matchId }: { matchId: string }) {
    const { paymentStatus, currentMatch } = useMatchStore();
    const { user } = useAuthStore();
    const navigate = useNavigate();
    const { t } = useTranslation();

    if (currentMatch?.status !== 'AWAITING_FUNDING') return null;

    const playerRole = getPlayerRoleForLobby(currentMatch, user?.id ?? null);
    const myPayment = getPaymentInfoForPlayer(paymentStatus, playerRole);
    const opPayment = paymentStatus
        ? (playerRole === 'A' ? paymentStatus.playerB : playerRole === 'B' ? paymentStatus.playerA : null)
        : null;

    return (
        <div className="mb-6 p-5 rounded-2xl border border-kaspa-primary/30 bg-kaspa-primary/5 space-y-3">
            <p className="text-[10px] font-black uppercase tracking-widest text-kaspa-primary mb-1">
                {t('match.status.waiting_deposits')}
            </p>

            {paymentStatus ? (
                <>
                    {/* My deposit status */}
                    <div className="flex items-center justify-between text-sm">
                        <span className="text-gray-400 font-bold">{t('match.status.me_player', { player: playerRole ?? '?' })}</span>
                        <span className={`font-black ${myPayment?.paid ? 'text-green-400' : 'text-yellow-400'}`}>
                            {myPayment?.paid
                                ? t('match.status.confirmed')
                                : myPayment?.payment_count
                                    ? <span className="flex items-center gap-1"><Icon name="clock" className="w-3 h-3" />{myPayment.min_confirmations}/{paymentStatus.min_confirmations_required} conf.</span>
                                    : t('match.status.not_deposited')}
                        </span>
                    </div>
                    {/* Opponent status */}
                    <div className="flex items-center justify-between text-sm">
                        <span className="text-gray-400 font-bold">{t('match.status.opponent_player', { player: playerRole === 'A' ? 'B' : playerRole === 'B' ? 'A' : '?' })}</span>
                        <span className={`font-black ${opPayment?.paid ? 'text-green-400' : 'text-gray-500'}`}>
                            {opPayment?.paid
                                ? t('match.status.confirmed')
                                : opPayment?.payment_count
                                    ? <span className="flex items-center gap-1"><Icon name="clock" className="w-3 h-3" />{opPayment.min_confirmations}/{paymentStatus.min_confirmations_required} conf.</span>
                                    : t('match.status.pending')}
                        </span>
                    </div>
                    {/* Required amount */}
                    <p className="text-[10px] text-gray-600 font-bold text-right">
                        {t('match.status.required', { amount: formatKas(paymentStatus.required_per_player_sompi), confirmations: paymentStatus.min_confirmations_required })}
                    </p>
                </>
            ) : (
                <p className="text-sm text-gray-500 font-bold animate-pulse">{t('match.status.loading_deposit')}</p>
            )}

            {/* Go to escrow button if user hasn't paid */}
            {playerRole && !myPayment?.paid && (
                <button
                    onClick={() => navigate(`/escrow/${matchId}`)}
                    className="w-full mt-2 h-10 rounded-xl bg-kaspa-primary/20 hover:bg-kaspa-primary/40 text-kaspa-primary font-black uppercase text-xs tracking-widest transition-colors"
                >
                    {t('match.status.deposit_now')}
                </button>
            )}
        </div>
    );
}

export function MatchPage() {
    const { matchId } = useParams<{ matchId: string }>();
    const { t } = useTranslation();

    useMatchPolling(matchId ?? null);
    usePaymentStatus(matchId ?? null);

    const { currentMatch, isLoading, error } = useMatchStore();

    if (error) {
        return (
            <div className="py-20 text-center">
                <Icon name="search" className="w-12 h-12 text-gray-700 mx-auto mb-4" />
                <h2 className="text-2xl font-bold text-red-500">{t('match.load_error')}</h2>
                <p className="text-gray-500 mt-2">{error}</p>
            </div>
        );
    }

    if (isLoading && !currentMatch) {
        return (
            <div className="py-20 flex flex-col items-center justify-center">
                <div className="animate-spin w-12 h-12 border-4 border-kaspa-primary border-t-transparent rounded-full mb-6" />
                <p className="text-gray-500 font-bold uppercase tracking-widest text-xs">{t('match.fetching_data')}</p>
            </div>
        );
    }

    if (!currentMatch) {
        return (
            <div className="py-20 text-center">
                <h2 className="text-4xl font-black opacity-20 italic">{t('match.not_found')}</h2>
            </div>
        );
    }

    return (
        <div>
            {matchId && <DepositStatusBanner matchId={matchId} />}
            {/* Provider decides the flow: NATIVE = browser game, FACEIT = existing FACEIT flow (unchanged). */}
            {getMatchProvider(currentMatch) === 'NATIVE'
                ? <NativeMatchView match={currentMatch} />
                : <MatchDetailView match={currentMatch} />}
        </div>
    );
}
