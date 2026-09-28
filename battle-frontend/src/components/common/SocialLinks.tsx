import { useTranslation } from 'react-i18next';
import { Icon, type IconName } from '../Icon';
import { publicLinks, socialLinks } from '../../config/publicLinks';

interface SocialLinksProps {
    className?: string;
    iconClassName?: string;
}

// Icon row for X / Instagram / Reddit / GitHub. Reddit is only included once
// verified reachable — see src/config/publicLinks.ts for the verification
// status and how to flip it on.
export function SocialLinks({ className = '', iconClassName = 'w-5 h-5' }: SocialLinksProps) {
    const { t } = useTranslation();

    const items: Array<{ key: string; href: string; label: string; icon: IconName }> = [
        { key: 'x', href: socialLinks.x.url, label: t('social.x'), icon: 'x-social' },
        { key: 'instagram', href: socialLinks.instagram.url, label: t('social.instagram'), icon: 'instagram' },
        ...(socialLinks.reddit.verified
            ? [{ key: 'reddit', href: socialLinks.reddit.url, label: t('social.reddit'), icon: 'reddit' as const }]
            : []),
        { key: 'github', href: publicLinks.githubRepository, label: t('social.github'), icon: 'github' },
    ];

    return (
        <div className={`flex items-center gap-3 ${className}`}>
            {items.map((item) => (
                <a
                    key={item.key}
                    href={item.href}
                    target="_blank"
                    rel="noopener noreferrer"
                    aria-label={item.label}
                    title={item.label}
                    className="w-10 h-10 flex items-center justify-center rounded-full glass-button text-gray-300 hover:text-kaspa-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kaspa-primary focus-visible:ring-offset-2 focus-visible:ring-offset-kaspa-dark transition-colors"
                >
                    <Icon name={item.icon} className={iconClassName} />
                </a>
            ))}
        </div>
    );
}
