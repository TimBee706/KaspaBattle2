import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AccountApiError, registerAccount } from '../api/account';
import { AuthCard, FormAlert } from '../components/auth/AuthCard';
import { FormField } from '../components/common/FormField';

export function RegisterPage() {
    const { t } = useTranslation();
    const [username, setUsername] = useState('');
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [passwordConfirm, setPasswordConfirm] = useState('');
    const [acceptTerms, setAcceptTerms] = useState(false);
    const [newsletter, setNewsletter] = useState(false);
    const [website, setWebsite] = useState(''); // honeypot
    const [busy, setBusy] = useState(false);
    const [fields, setFields] = useState<Record<string, string>>({});
    const [formError, setFormError] = useState<string | null>(null);
    const [done, setDone] = useState<null | { verification: boolean }>(null);

    const fieldError = (name: string) => (fields[name] ? t(`auth.field_error.${fields[name]}`, { defaultValue: t('auth.field_error.invalid') }) : undefined);

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (busy) return;
        setBusy(true);
        setFields({});
        setFormError(null);
        try {
            const res = await registerAccount({ username, email, password, passwordConfirm, acceptTerms, newsletter, website });
            setDone({ verification: res.emailVerificationRequired });
        } catch (err) {
            const e2 = err as AccountApiError;
            if (e2.code === 'username_taken') setFields({ username: 'taken' });
            else if (e2.code === 'invalid_input') setFields(e2.fields);
            else if (e2.code === 'rate_limited') setFormError(t('auth.errors.rate_limited'));
            else setFormError(t('auth.errors.generic'));
        } finally {
            setBusy(false);
        }
    };

    if (done) {
        return (
            <AuthCard title={t('auth.register.done_title')}>
                <FormAlert kind="success">{done.verification ? t('auth.register.done_verify') : t('auth.register.done_login')}</FormAlert>
                <Link to="/login" className="btn-primary flex h-11 items-center justify-center">
                    {t('auth.login.submit')}
                </Link>
            </AuthCard>
        );
    }

    return (
        <AuthCard
            title={t('auth.register.title')}
            subtitle={t('auth.register.subtitle')}
            footer={<>{t('auth.register.have_account')} <Link to="/login" className="font-bold text-kaspa-primary hover:underline">{t('auth.login.submit')}</Link></>}
        >
            <form onSubmit={submit} className="space-y-4" noValidate>
                <FormField label={t('auth.username')} htmlFor="reg-username" error={fieldError('username')} hint={t('auth.username_hint')}>
                    <input id="reg-username" className={`form-input ${fields.username ? 'form-input-error' : ''}`} autoComplete="username" value={username} onChange={(e) => setUsername(e.target.value)} maxLength={24} required />
                </FormField>
                <FormField label={t('auth.email')} htmlFor="reg-email" error={fieldError('email')}>
                    <input id="reg-email" type="email" className={`form-input ${fields.email ? 'form-input-error' : ''}`} autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
                </FormField>
                <FormField label={t('auth.password')} htmlFor="reg-password" error={fieldError('password')} hint={t('auth.password_hint')}>
                    <input id="reg-password" type="password" className={`form-input ${fields.password ? 'form-input-error' : ''}`} autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
                </FormField>
                <FormField label={t('auth.password_confirm')} htmlFor="reg-password2" error={fieldError('passwordConfirm')}>
                    <input id="reg-password2" type="password" className={`form-input ${fields.passwordConfirm ? 'form-input-error' : ''}`} autoComplete="new-password" value={passwordConfirm} onChange={(e) => setPasswordConfirm(e.target.value)} required />
                </FormField>

                {/* Honeypot: invisible to people, irresistible to bots. */}
                <div aria-hidden="true" className="absolute -left-[9999px] h-0 w-0 overflow-hidden">
                    <label>Website <input tabIndex={-1} autoComplete="off" value={website} onChange={(e) => setWebsite(e.target.value)} /></label>
                </div>

                <label className="flex items-start gap-3 text-sm text-gray-300">
                    <input type="checkbox" className="mt-1 h-4 w-4 accent-[#49EACB]" checked={acceptTerms} onChange={(e) => setAcceptTerms(e.target.checked)} />
                    <span>
                        {t('auth.register.accept_terms_prefix')}{' '}
                        <Link to="/terms" className="font-bold text-kaspa-primary hover:underline">{t('auth.register.terms_link')}</Link>
                    </span>
                </label>
                {fields.acceptTerms && <p className="form-error">{t('auth.field_error.required')}</p>}
                <label className="flex items-start gap-3 text-sm text-gray-400">
                    <input type="checkbox" className="mt-1 h-4 w-4 accent-[#49EACB]" checked={newsletter} onChange={(e) => setNewsletter(e.target.checked)} />
                    <span>{t('auth.register.newsletter')}</span>
                </label>

                {formError && <FormAlert kind="error">{formError}</FormAlert>}
                <button type="submit" disabled={busy} className="btn-primary h-12 w-full">
                    {busy ? t('common.loading') : t('auth.register.submit')}
                </button>
            </form>
        </AuthCard>
    );
}
