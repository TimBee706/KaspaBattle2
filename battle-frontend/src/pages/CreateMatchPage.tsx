import { CreateChallengeForm } from '../components/match/CreateChallengeForm';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';

export function CreateMatchPage() {
    const { t } = useTranslation();

    return (
        <div className="container mx-auto px-4 py-8 animate-in fade-in slide-in-from-top-4 duration-500">
            {/* Header — mirrors LobbyPage exactly */}
            <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('create_match.title')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('create_match.subtitle')}</p>
                </div>
            </div>

            <CreateChallengeForm />

            <div className="max-w-lg mx-auto mt-6 grid grid-cols-2 gap-4">
                <div className="bg-slate-900/60 border border-slate-700/50 rounded-xl p-4 flex flex-col items-center hover:-translate-y-0.5 transition-transform">
                    <Icon name="bolt" className="w-5 h-5 text-kaspa-primary mb-2" />
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold text-center">{t('create_match.instant')}</p>
                </div>
                <div className="bg-slate-900/60 border border-slate-700/50 rounded-xl p-4 flex flex-col items-center hover:-translate-y-0.5 transition-transform">
                    <Icon name="lock" className="w-5 h-5 text-kaspa-primary mb-2" />
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold text-center">{t('create_match.escrow')}</p>
                </div>
            </div>
        </div>
    );
}
