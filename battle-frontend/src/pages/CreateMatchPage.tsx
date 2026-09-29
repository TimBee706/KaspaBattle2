import { useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { CreateChallengeForm } from '../components/match/CreateChallengeForm';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';
import { PageHeader } from '../components/common/PageHeader';
import { KaspaCoin } from '../components/native/ConnectFourCell';
import { useNativeGamesEnabled } from '../hooks/useNativeGamesEnabled';
import type { MatchProvider } from '../api/types';

export function CreateMatchPage() {
    const { t } = useTranslation();
    const nativeEnabled = useNativeGamesEnabled();
    const [params] = useSearchParams();
    const wantsNative = params.get('provider')?.toLowerCase() === 'native';
    // The player's explicit choice wins; otherwise ?provider=native preselects browser games.
    // Native is only ever effective while the backend feature flag is on.
    const [choice, setChoice] = useState<MatchProvider | null>(null);
    const provider: MatchProvider = nativeEnabled ? (choice ?? (wantsNative ? 'NATIVE' : 'FACEIT')) : 'FACEIT';
    const setProvider = setChoice;

    const options: Array<{ value: MatchProvider; title: string; subtitle: string; disabled?: boolean }> = [
        { value: 'NATIVE', title: t('create_match.provider.native.title'), subtitle: t('create_match.provider.native.subtitle'), disabled: !nativeEnabled },
        { value: 'FACEIT', title: t('create_match.provider.faceit.title'), subtitle: t('create_match.provider.faceit.subtitle') },
    ];

    return (
        <div>
            <PageHeader icon="bolt" title={t('create_match.title')} subtitle={t('create_match.subtitle')} />

            <div className="max-w-lg mx-auto">
                <div role="group" aria-label={t('create_match.provider.label')} className="mb-4 grid grid-cols-1 sm:grid-cols-2 gap-3">
                    {options.map((opt) => (
                        <button
                            key={opt.value}
                            type="button"
                            id={`provider-${opt.value.toLowerCase()}`}
                            disabled={opt.disabled}
                            aria-pressed={provider === opt.value}
                            onClick={() => setProvider(opt.value)}
                            className={[
                                'rounded-2xl border p-4 text-left transition-all disabled:cursor-not-allowed disabled:opacity-40',
                                provider === opt.value
                                    ? 'border-kaspa-primary bg-kaspa-primary/10 shadow-glow-subtle'
                                    : 'border-kaspa-border bg-kaspa-card/60 hover:border-gray-600',
                            ].join(' ')}
                        >
                            <span className="flex items-center gap-2 text-sm font-black uppercase tracking-tight text-white">
                                {opt.value === 'NATIVE'
                                    ? <span className="flex gap-0.5"><KaspaCoin color="blue" className="h-5 w-5" /><KaspaCoin color="red" className="h-5 w-5" /></span>
                                    : <Icon name="link" className="h-4 w-4 text-[#FF5500]" />}
                                {opt.title}
                            </span>
                            <span className="mt-1 block text-xs text-gray-400">{opt.subtitle}</span>
                        </button>
                    ))}
                </div>
                {!nativeEnabled && (
                    <p className="mb-4 text-center text-2xs font-bold uppercase tracking-widest text-gray-500" data-testid="native-disabled-note">
                        {t('create_match.provider.native_disabled')}
                    </p>
                )}

                {/* key resets the form (selected game, mode) when the provider changes */}
                <CreateChallengeForm key={provider} provider={provider} />

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
