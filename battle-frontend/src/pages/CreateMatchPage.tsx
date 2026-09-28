import { CreateChallengeForm } from '../components/match/CreateChallengeForm';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';
import { PageHeader } from '../components/common/PageHeader';

export function CreateMatchPage() {
    const { t } = useTranslation();

    return (
        <div>
            <PageHeader icon="bolt" title={t('create_match.title')} subtitle={t('create_match.subtitle')} />

            <div className="max-w-lg mx-auto">
                <CreateChallengeForm />

                <div className="mt-6 grid grid-cols-2 gap-4">
                    <div className="bg-kaspa-card/60 border border-kaspa-border rounded-2xl p-4 flex flex-col items-center hover:-translate-y-0.5 transition-transform">
                        <Icon name="bolt" className="w-5 h-5 text-kaspa-primary mb-2" />
                        <p className="text-2xs text-gray-500 uppercase tracking-widest font-bold text-center">{t('create_match.instant')}</p>
                    </div>
                    <div className="bg-kaspa-card/60 border border-kaspa-border rounded-2xl p-4 flex flex-col items-center hover:-translate-y-0.5 transition-transform">
                        <Icon name="lock" className="w-5 h-5 text-kaspa-primary mb-2" />
                        <p className="text-2xs text-gray-500 uppercase tracking-widest font-bold text-center">{t('create_match.escrow')}</p>
                    </div>
                </div>
            </div>
        </div>
    );
}
