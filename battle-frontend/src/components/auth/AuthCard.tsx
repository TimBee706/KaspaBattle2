import type { ReactNode } from 'react';

/** Shared frame of the sign-up / sign-in / recovery screens (existing glass design). */
export function AuthCard({ title, subtitle, children, footer }: { title: string; subtitle?: string; children: ReactNode; footer?: ReactNode }) {
    return (
        <div className="mx-auto w-full max-w-md py-8">
            <div className="glass-panel space-y-6 p-6 sm:p-8">
                <div>
                    <h1 className="text-2xl font-black uppercase tracking-tight text-white">{title}</h1>
                    {subtitle && <p className="mt-2 text-sm text-gray-400">{subtitle}</p>}
                </div>
                {children}
            </div>
            {footer && <div className="mt-5 text-center text-sm text-gray-400">{footer}</div>}
        </div>
    );
}

export function FormAlert({ kind, children }: { kind: 'error' | 'success' | 'info'; children: ReactNode }) {
    const tone =
        kind === 'error'
            ? 'border-red-500/30 bg-red-900/20 text-red-300'
            : kind === 'success'
                ? 'border-emerald-500/30 bg-emerald-900/20 text-emerald-300'
                : 'border-kaspa-primary/30 bg-kaspa-primary/10 text-kaspa-primary';
    return (
        <p role={kind === 'error' ? 'alert' : 'status'} className={`rounded-lg border p-3 text-sm font-semibold ${tone}`}>
            {children}
        </p>
    );
}
