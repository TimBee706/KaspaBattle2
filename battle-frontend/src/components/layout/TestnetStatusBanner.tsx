import { useTranslation } from 'react-i18next';
import { Icon } from '../Icon';

// Global, persistent status strip — rendered above the Header in Layout.tsx
// so it's visible on every page, desktop and mobile. Intentionally quiet
// (no red/yellow alert styling): this communicates ongoing status, not an
// error condition.
export function TestnetStatusBanner() {
    const { t } = useTranslation();

    return (
        <div className="bg-kaspa-surface/90 backdrop-blur-glass border-b border-kaspa-primary/15">
            <div className="container mx-auto px-4 py-2 flex flex-wrap items-center justify-center gap-x-3 gap-y-1 text-center text-xs md:text-sm">
                <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-kaspa-primary/15 border border-kaspa-primary/30 text-kaspa-primary font-black tracking-wider text-[10px] uppercase shrink-0">
                    <span className="relative flex h-1.5 w-1.5">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-kaspa-primary opacity-75" />
                        <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-kaspa-primary" />
                    </span>
                    {t('testnet_banner.badge')}
                </span>
                <span className="text-white font-bold shrink-0">{t('testnet_banner.status')}</span>
                <span className="text-gray-400 font-medium">{t('testnet_banner.message')}</span>
                <a
                    href="#testnet-details"
                    className="text-kaspa-primary hover:underline font-semibold shrink-0 inline-flex items-center gap-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kaspa-primary rounded"
                >
                    {t('testnet_banner.details_link')}
                    <Icon name="chevron-right" className="w-3 h-3" />
                </a>
            </div>
        </div>
    );
}
