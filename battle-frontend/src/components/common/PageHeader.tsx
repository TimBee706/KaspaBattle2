import type { ReactNode } from 'react';
import type { IconName } from '../Icon';
import { Icon } from '../Icon';

interface PageHeaderProps {
    title: ReactNode;
    subtitle?: ReactNode;
    icon?: IconName;
    actions?: ReactNode;
    className?: string;
}

// Formalizes the page-title + eyebrow-subtitle pattern that was previously
// hand-copied verbatim (text-4xl font-black ... / text-slate-500 text-sm ...)
// across Lobby, Create Match, Wallet, History, Tournaments and Profile.
// Deliberately kept smaller than the landing-page hero — functional views
// don't need hero-scale type.
export function PageHeader({ title, subtitle, icon, actions, className = '' }: PageHeaderProps) {
    return (
        <div className={`flex flex-col md:flex-row md:justify-between md:items-center gap-6 mb-10 ${className}`}>
            <div className="flex items-center gap-4">
                {icon && (
                    <div className="hidden sm:flex w-12 h-12 shrink-0 rounded-xl bg-kaspa-primary/10 border border-kaspa-primary/20 items-center justify-center">
                        <Icon name={icon} className="w-6 h-6 text-kaspa-primary" />
                    </div>
                )}
                <div>
                    <h1 className="text-3xl md:text-4xl font-black text-white uppercase tracking-tighter">
                        {title}
                    </h1>
                    {subtitle && (
                        <p className="text-gray-500 text-sm font-bold uppercase tracking-widest mt-1.5">
                            {subtitle}
                        </p>
                    )}
                </div>
            </div>
            {actions && <div className="flex items-center gap-3 w-full md:w-auto shrink-0">{actions}</div>}
        </div>
    );
}
