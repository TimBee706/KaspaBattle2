import { useTranslation } from 'react-i18next';

interface PublicBetaBadgeProps {
    size?: 'sm' | 'lg';
    className?: string;
}

// Shared pulse-dot "Public Testnet Beta" pill — used in both the global
// TestnetStatusBanner and the landing-page hero eyebrow, so the same visual
// signal appears consistently everywhere it's shown.
export function PublicBetaBadge({ size = 'lg', className = '' }: PublicBetaBadgeProps) {
    const { t } = useTranslation();
    const isSmall = size === 'sm';

    return (
        <div
            className={`inline-flex items-center gap-1.5 rounded-full bg-kaspa-primary/10 border border-kaspa-primary/20 text-kaspa-primary font-black uppercase tracking-widest ${
                isSmall ? 'px-2 py-0.5 text-[9px]' : 'px-3 py-1 text-[10px]'
            } ${className}`}
        >
            <span className={`relative flex ${isSmall ? 'h-1.5 w-1.5' : 'h-2 w-2'}`}>
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-kaspa-primary opacity-75" />
                <span className={`relative inline-flex rounded-full bg-kaspa-primary ${isSmall ? 'h-1.5 w-1.5' : 'h-2 w-2'}`} />
            </span>
            {t('hero.beta_tag')}
        </div>
    );
}
