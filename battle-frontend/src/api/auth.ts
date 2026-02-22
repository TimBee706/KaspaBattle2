import apiClient from './client';
import { FACEIT_CLIENT_ID, FACEIT_AUTH_URL, FACEIT_REDIRECT_URI, FACEIT_SCOPES } from '../config/constants';
import type { FaceitAuthResponse } from './types';

// PKCE Code Verifier generieren
function generateCodeVerifier(): string {
    const array = new Uint8Array(32);
    crypto.getRandomValues(array);
    return btoa(String.fromCharCode(...array))
        .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

// PKCE Code Challenge aus Verifier
async function generateCodeChallenge(verifier: string): Promise<string> {
    const encoder = new TextEncoder();
    const data = encoder.encode(verifier);
    const digest = await crypto.subtle.digest('SHA-256', data);
    return btoa(String.fromCharCode(...new Uint8Array(digest)))
        .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

// OAuth Login starten – leitet zu FACEIT weiter
export async function startFaceitLogin(): Promise<void> {
    const codeVerifier = generateCodeVerifier();
    const codeChallenge = await generateCodeChallenge(codeVerifier);
    const state = crypto.randomUUID();

    // Verifier + State im SessionStorage speichern
    sessionStorage.setItem('faceit_code_verifier', codeVerifier);
    sessionStorage.setItem('faceit_state', state);

    const params = new URLSearchParams({
        client_id: FACEIT_CLIENT_ID,
        response_type: 'code',
        redirect_uri: FACEIT_REDIRECT_URI,
        scope: FACEIT_SCOPES,
        state: state,
        code_challenge: codeChallenge,
        code_challenge_method: 'S256',
    });

    window.location.href = `${FACEIT_AUTH_URL}/authorize?${params.toString()}`;
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
