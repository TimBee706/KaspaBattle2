import { useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AccountApiError, resetPassword } from '../api/account';
import { AuthCard, FormAlert } from '../components/auth/AuthCard';
import { FormField } from '../components/common/FormField';

export function ResetPasswordPage() {
    const { t } = useTranslation();
    const [params] = useSearchParams();
    const token = params.get('token') ?? '';
    const [password, setPassword] = useState('');
    const [confirm, setConfirm] = useState('');
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [fieldCode, setFieldCode] = useState<string | null>(null);
    const [done, setDone] = useState(false);

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (busy) return;
        setError(null);
        setFieldCode(null);
        if (password !== confirm) {
            setFieldCode('mismatch');
            return;
        }
        setBusy(true);
        try {
            await resetPassword(token, password);
            setDone(true);
        } catch (err) {
            const e2 = err as AccountApiError;
            if (e2.code === 'invalid_input') setFieldCode(e2.fields.newPassword ?? 'invalid');
            else if (e2.code === 'invalid_or_expired_token') setError(t('auth.reset.invalid_token'));
            else setError(t('auth.errors.generic'));
        } finally {
            setBusy(false);
        }
    };

    if (!token) {
        return (
            <AuthCard title={t('auth.reset.title')}>
                <FormAlert kind="error">{t('auth.reset.invalid_token')}</FormAlert>
                <Link to="/forgot-password" className="text-sm font-bold text-kaspa-primary hover:underline">{t('auth.forgot.title')}</Link>
            </AuthCard>
        );
    }
    if (done) {
        return (
            <AuthCard title={t('auth.reset.title')}>
                <FormAlert kind="success">{t('auth.reset.done')}</FormAlert>
                <Link to="/login" className="btn-primary flex h-11 items-center justify-center">{t('auth.login.submit')}</Link>
            </AuthCard>
        );
    }
    return (
        <AuthCard title={t('auth.reset.title')} subtitle={t('auth.reset.subtitle')}>
            <form onSubmit={submit} className="space-y-4" noValidate>
                <FormField label={t('auth.password_new')} htmlFor="reset-password" error={fieldCode ? t(`auth.field_error.${fieldCode}`, { defaultValue: t('auth.field_error.invalid') }) : undefined} hint={t('auth.password_hint')}>
                    <input id="reset-password" type="password" className="form-input" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
                </FormField>
                <FormField label={t('auth.password_confirm')} htmlFor="reset-password2">
                    <input id="reset-password2" type="password" className="form-input" autoComplete="new-password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required />
                </FormField>
                {error && <FormAlert kind="error">{error}</FormAlert>}
                <button type="submit" disabled={busy} className="btn-primary h-12 w-full">{busy ? t('common.loading') : t('auth.reset.submit')}</button>
            </form>
        </AuthCard>
    );
}
