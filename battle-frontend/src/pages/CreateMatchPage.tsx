import { CreateChallengeForm } from '../components/match/CreateChallengeForm';
import { useTranslation } from 'react-i18next';

export function CreateMatchPage() {
    const { t } = useTranslation();

    return (
        <div className="py-10 animate-in fade-in slide-in-from-top-4 duration-500">
            <div className="max-w-xl mx-auto mb-10 text-center">
                <h1 className="text-4xl font-black mb-4 tracking-tight">{t('create_match.title')}</h1>
                <p className="text-gray-400">
                    {t('create_match.subtitle')}
                </p>
            </div>

            <CreateChallengeForm />

            <div className="max-w-lg mx-auto mt-12 grid grid-cols-1 md:grid-cols-2 gap-6 text-center">
                <div className="p-4 bg-kaspa-card/30 rounded-xl border border-kaspa-border">
                    <span className="text-kaspa-primary text-xl block mb-2">⚡</span>
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold">{t('create_match.instant')}</p>
                </div>
                <div className="p-4 bg-kaspa-card/30 rounded-xl border border-kaspa-border">
                    <span className="text-kaspa-primary text-xl block mb-2">🔒</span>
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold">{t('create_match.escrow')}</p>
                </div>
            </div>
        </div>
    );
}
