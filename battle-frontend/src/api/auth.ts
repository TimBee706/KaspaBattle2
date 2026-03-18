import apiClient from './client';
import { API_BASE_URL, FACEIT_REDIRECT_URI } from '../config/constants';
import type { FaceitAuthResponse } from './types';

// OAuth Login starten – leitet über unser Backend zum FACEIT SSO weiter
export function startFaceitLogin(): void {
    window.location.href = `${API_BASE_URL}/faceit/login`;
}

// OAuth Profil Verknüpfen starten (wenn bereits eingeloggt)
export function startFaceitLink(): void {
    window.location.href = `${API_BASE_URL}/faceit/link`;
}

// OAuth Callback verarbeiten – tauscht Code gegen Tokens
export async function handleFaceitCallback(
    code: string,
    state: string,
): Promise<FaceitAuthResponse> {
    const savedState = sessionStorage.getItem('faceit_state');
    const codeVerifier = sessionStorage.getItem('faceit_code_verifier');

    if (state !== savedState) {
        throw new Error('OAuth state mismatch – möglicher CSRF-Angriff');
    }
    if (!codeVerifier) {
        throw new Error('Code verifier nicht gefunden – bitte erneut einloggen');
    }

    // Cleanup
    sessionStorage.removeItem('faceit_state');
    sessionStorage.removeItem('faceit_code_verifier');

    // Backend tauscht Code gegen Tokens + erstellt/aktualisiert User-Profil
    const res = await apiClient.post<FaceitAuthResponse>('/auth/faceit/callback', {
        code,
        code_verifier: codeVerifier,
        redirect_uri: FACEIT_REDIRECT_URI,
    });

    return res.data;
}

// Token erneuern
export async function refreshToken(refreshToken: string): Promise<FaceitAuthResponse> {
    const res = await apiClient.post<FaceitAuthResponse>('/auth/refresh', {
        refresh_token: refreshToken,
    });
    return res.data;
}

export async function logout(): Promise<void> {
    await apiClient.post('/auth/logout');
}
