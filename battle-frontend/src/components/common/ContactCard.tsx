import { useTranslation } from 'react-i18next';
import { Icon } from '../Icon';
import { publicLinks } from '../../config/publicLinks';
import { PUBLIC_CONTACT_EMAIL } from '../../config/constants';

interface ContactCardProps {
    className?: string;
}

// Renders a mailto: CTA only when VITE_PUBLIC_CONTACT_EMAIL is configured;
// otherwise falls back to a GitHub "new issue" CTA so there's always a way
// to reach the project.
export function ContactCard({ className = '' }: ContactCardProps) {
    const { t } = useTranslation();
    const hasEmail = Boolean(PUBLIC_CONTACT_EMAIL);

    return (
        <div className={`glass-panel rounded-2xl p-6 flex flex-col sm:flex-row items-start sm:items-center gap-4 ${className}`}>
            <div className="w-11 h-11 rounded-xl bg-kaspa-primary/10 border border-kaspa-primary/20 flex items-center justify-center shrink-0">
                <Icon name={hasEmail ? 'mail' : 'message-circle'} className="w-5 h-5 text-kaspa-primary" />
            </div>
            <p className="text-sm text-gray-400 flex-1">
                {hasEmail ? t('footer.contact_email_cta') : t('footer.contact_github_fallback')}
            </p>
            {hasEmail ? (
                <a
                    href={`mailto:${PUBLIC_CONTACT_EMAIL}`}
                    className="glass-button px-5 py-2.5 text-sm font-semibold inline-flex items-center gap-2 shrink-0"
                >
                    <Icon name="mail" className="w-4 h-4" />
                    {PUBLIC_CONTACT_EMAIL}
                </a>
            ) : (
                <a
                    href={publicLinks.githubIssueNew}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="glass-button px-5 py-2.5 text-sm font-semibold inline-flex items-center gap-2 shrink-0"
                >
                    <Icon name="github" className="w-4 h-4" />
                    {t('sections.recruit.feedback.cta_issue')}
                </a>
            )}
        </div>
    );
}
