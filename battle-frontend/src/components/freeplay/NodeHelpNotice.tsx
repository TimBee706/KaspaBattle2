import { useTranslation } from 'react-i18next';
import { Icon } from '../Icon';
import { NODE_HELP_CONTACT_EMAIL } from '../../config/constants';

/** Landing-page notice: Free Play is live, the on-chain testnet mode needs a reliable node. */
export function NodeHelpNotice() {
    const { t } = useTranslation();
    return (
        <aside
            aria-labelledby="node-help-title"
            data-testid="node-help-notice"
            className="glass-panel relative mx-auto max-w-4xl overflow-hidden rounded-2xl border border-kaspa-primary/30 p-6 md:p-8"
        >
            <div aria-hidden="true" className="pointer-events-none absolute -right-10 -top-10 h-48 w-48 rounded-full bg-kaspa-primary/10 blur-3xl" />
            <div className="relative flex flex-col gap-4 md:flex-row md:items-start">
                <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl border border-kaspa-primary/20 bg-kaspa-primary/10">
                    <Icon name="bolt" className="h-6 w-6 text-kaspa-primary" />
                </div>
                <div className="min-w-0 flex-1 space-y-3">
                    <h2 id="node-help-title" className="text-xl font-black uppercase tracking-tight text-white">
                        {t('freeplay.banner.title')}
                    </h2>
                    <p className="text-sm leading-relaxed text-gray-300">{t('freeplay.banner.text')}</p>
                    <p className="text-sm leading-relaxed text-gray-400">
                        {t('freeplay.banner.help')}{' '}
                        <a
                            href={`mailto:${NODE_HELP_CONTACT_EMAIL}`}
                            className="font-bold text-kaspa-primary underline-offset-2 hover:underline"
                        >
                            {NODE_HELP_CONTACT_EMAIL}
                        </a>
                    </p>
                    <p className="inline-flex items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-900/10 px-3 py-1.5 text-xs font-bold text-amber-300">
                        <Icon name="clock" className="h-3.5 w-3.5" />
                        {t('freeplay.testnet.status')}
                    </p>
                </div>
            </div>
        </aside>
    );
}
