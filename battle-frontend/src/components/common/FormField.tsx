import type { ReactNode } from 'react';
import { Icon } from '../Icon';

interface FormFieldProps {
    label: ReactNode;
    htmlFor?: string;
    hint?: ReactNode;
    error?: string;
    children: ReactNode;
    className?: string;
}

// Consistent label + control + hint/error wrapper. Pairs with the
// .form-label/.form-input/.form-hint/.form-error classes in globals.css.
// Renders the error in place of the hint (never both) so validation
// feedback doesn't get lost among static help text.
export function FormField({ label, htmlFor, hint, error, children, className = '' }: FormFieldProps) {
    return (
        <div className={className}>
            <label htmlFor={htmlFor} className="form-label">
                {label}
            </label>
            {children}
            {error ? (
                <p className="form-error">
                    <Icon name="alert-triangle" className="w-3.5 h-3.5 shrink-0" />
                    {error}
                </p>
            ) : hint ? (
                <p className="form-hint">{hint}</p>
            ) : null}
        </div>
    );
}
