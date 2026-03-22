import apiClient from './client';
import { FACEIT_REDIRECT_URI } from '../config/constants';
import type { FaceitAuthResponse } from './types';

// OAuth Login starten – holt FACEIT Auth-URL als JSON und navigiert direkt dorthin.
// WICHTIG: Nicht über Vite-Proxy redirecten, da der Proxy den 307 serverseitig folgt
// und der Browser dann auf dem ngrok-URL bleibt → FACEIT zeigt "Fenster schließen".
export async function startFaceitLogin(): Promise<void> {
    const res = await apiClient.get<{ url: string }>('/faceit/auth-url');
    window.location.href = res.data.url;
}

// OAuth Profil Verknüpfen starten (wenn bereits eingeloggt)
export async function startFaceitLink(): Promise<void> {
    const res = await apiClient.get<{ url: string }>('/faceit/auth-url');
    window.location.href = res.data.url;
}

// OAuth Callback verarbeiten – tauscht Code gegen Tokens (dead code, Callback läuft server-seitig)
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
