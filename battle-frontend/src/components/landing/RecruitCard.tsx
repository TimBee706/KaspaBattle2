import type { IconName } from '../Icon';
import { Icon } from '../Icon';

interface RecruitCta {
    label: string;
    href?: string;
    onClick?: () => void;
    external?: boolean;
}

interface RecruitCardProps {
    icon: IconName;
    title: string;
    audience: string;
    text: string;
    ctas: RecruitCta[];
}

export function RecruitCard({ icon, title, audience, text, ctas }: RecruitCardProps) {
    return (
        <div className="glass-panel rounded-2xl border border-kaspa-primary/15 p-8 flex flex-col h-full hover:border-kaspa-primary/35 hover:shadow-glow-subtle hover:-translate-y-1 transition-all duration-300">
            <div className="w-12 h-12 rounded-xl bg-kaspa-primary/10 border border-kaspa-primary/20 flex items-center justify-center mb-5">
                <Icon name={icon} className="w-6 h-6 text-kaspa-primary" />
            </div>
            <h3 className="text-xl font-bold text-white mb-1">{title}</h3>
            <p className="text-xs text-kaspa-primary/80 font-semibold uppercase tracking-wide mb-4">{audience}</p>
            <p className="text-gray-400 text-sm leading-relaxed mb-6 flex-1">{text}</p>
            <div className="flex flex-wrap gap-4 mt-auto">
                {ctas.map((cta, i) => {
                    const isPrimary = i === 0;
                    const className = isPrimary
                        ? 'glass-button px-5 py-2.5 text-sm font-semibold inline-flex items-center gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kaspa-primary'
                        : 'text-sm font-semibold text-kaspa-primary hover:underline inline-flex items-center gap-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kaspa-primary rounded';

                    if (cta.href) {
                        return (
                            <a
                                key={cta.label}
                                href={cta.href}
                                {...(cta.external ? { target: '_blank', rel: 'noopener noreferrer' } : {})}
                                className={className}
                            >
                                {cta.label}
                                {cta.external && <Icon name="external-link" className="w-3.5 h-3.5" />}
                            </a>
                        );
                    }
                    return (
                        <button key={cta.label} type="button" onClick={cta.onClick} className={className}>
                            {cta.label}
                        </button>
                    );
                })}
            </div>
        </div>
    );
}
