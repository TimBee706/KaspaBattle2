import type { ReactNode } from 'react';

interface SectionHeadingProps {
    eyebrow?: ReactNode;
    title: ReactNode;
    subtitle?: ReactNode;
    align?: 'center' | 'left';
    className?: string;
}

// Reusable landing-page section heading — matches the underline-decoration
// style already used for the existing "WHAT IS KASPABATTLE?" / "WHY KASPA?"
// headings, generalized with an optional eyebrow label and subtitle line.
export function SectionHeading({ eyebrow, title, subtitle, align = 'center', className = '' }: SectionHeadingProps) {
    const wrapperAlign = align === 'center' ? 'text-center mx-auto' : 'text-left';

    return (
        <div className={`max-w-3xl mb-12 md:mb-16 px-4 ${wrapperAlign} ${className}`}>
            {eyebrow && (
                <p className="text-kaspa-primary text-2xs font-black uppercase tracking-[0.2em] mb-3">
                    {eyebrow}
                </p>
            )}
            <h2 className="text-3xl md:text-4xl font-black uppercase tracking-tight underline decoration-kaspa-primary decoration-4 underline-offset-8">
                {title}
            </h2>
            {subtitle && (
                <p className="mt-6 text-lg text-gray-400 font-medium leading-relaxed">{subtitle}</p>
            )}
        </div>
    );
}
