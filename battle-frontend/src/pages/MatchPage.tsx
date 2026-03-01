import { useParams } from 'react-router-dom';
import { MatchDetailView } from '../components/match/MatchDetailView';
import { useMatchPolling } from '../hooks/useMatchPolling';
import { useMatchStore } from '../stores/useMatchStore';
import { useTranslation } from 'react-i18next';

export function MatchPage() {
    const { matchId } = useParams<{ matchId: string }>();
    useMatchPolling(matchId ?? null);
    const { t } = useTranslation();

    const { currentMatch, isLoading, error } = useMatchStore();

    if (error) {
        return (
            <div className="py-20 text-center">
                <div className="text-5xl mb-4">🔍</div>
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

    return <MatchDetailView match={currentMatch} />;
}
