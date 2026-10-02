import apiClient from './client';

export interface FieldErrors {
    [field: string]: string;
}

/** Error thrown by account calls; carries the server's stable error code and per-field codes. */
export class AccountApiError extends Error {
    code: string;
    status: number;
    fields: FieldErrors;
    constructor(code: string, status: number, fields: FieldErrors = {}) {
        super(code);
        this.code = code;
        this.status = status;
        this.fields = fields;
    }
}

function toApiError(err: unknown): AccountApiError {
    const e = err as { response?: { status?: number; data?: { error?: string; fields?: FieldErrors } } };
    return new AccountApiError(e.response?.data?.error ?? 'network_error', e.response?.status ?? 0, e.response?.data?.fields ?? {});
}

async function call<T>(promise: Promise<{ data: T }>): Promise<T> {
    try {
        return (await promise).data;
    } catch (err) {
        throw toApiError(err);
    }
}

export interface RegisterInput {
    username: string;
    email: string;
    password: string;
    passwordConfirm: string;
    acceptTerms: boolean;
    newsletter: boolean;
    /** honeypot – must stay empty */
    website: string;
}

export const registerAccount = (b: RegisterInput) =>
    call<{ status: 'registered'; emailVerificationRequired: boolean }>(apiClient.post('/auth/register', b));

export const loginWithPassword = (email: string, password: string, remember: boolean) =>
    call<{ userId: string; username: string }>(apiClient.post('/auth/login', { email, password, remember }));

export const verifyEmail = (token: string) => call<{ verified: true }>(apiClient.post('/auth/verify-email', { token }));
export const resendVerification = (email: string) => call<{ status: 'ok' }>(apiClient.post('/auth/resend-verification', { email }));
export const forgotPassword = (email: string) => call<{ status: 'ok' }>(apiClient.post('/auth/forgot-password', { email }));
export const resetPassword = (token: string, newPassword: string) =>
    call<{ status: 'ok' }>(apiClient.post('/auth/reset-password', { token, newPassword }));
export const changePassword = (currentPassword: string, newPassword: string) =>
    call<{ status: 'ok' }>(apiClient.post('/auth/change-password', { currentPassword, newPassword }));
export const setNewsletter = (subscribe: boolean) => call<{ subscribed: boolean }>(apiClient.post('/auth/newsletter', { subscribe }));
