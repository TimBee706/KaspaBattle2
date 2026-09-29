import { useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';

/**
 * Fallback for environments where the reverse proxy does NOT route
 * `/auth/faceit/callback` straight to the backend (e.g. local Vite dev).
 *
 * In production Caddy sends that path to the backend, which redeems the code server-side,
 * sets the session cookie and answers 303 -> /lobby?linked=1 (this component never renders).
 * Here we only hand the query over to the same-origin backend endpoint, exactly once.
 * The code/state are never logged.
 */
export function FaceitCallback() {
    const [searchParams] = useSearchParams();

    useEffect(() => {
        const code = searchParams.get('code');
        const state = searchParams.get('state');
        const errorParam = searchParams.get('error');

        if (errorParam) {
            window.location.replace('/?error=faceit_denied');
            return;
        }
        if (!code || !state) {
            window.location.replace('/?error=faceit_invalid_callback');
            return;
        }

        // React StrictMode runs effects twice; an OAuth code may only be redeemed once.
        const dedupeKey = `oauth_bounce_${state}`;
        if (sessionStorage.getItem(dedupeKey)) return;
        sessionStorage.setItem(dedupeKey, '1');

        const apiBase = import.meta.env.VITE_API_BASE_URL || '/api/v1';
        window.location.replace(
            `${apiBase}/faceit/callback?code=${encodeURIComponent(code)}&state=${encodeURIComponent(state)}`,
        );
    }, [searchParams]);

    // Show a loading spinner while the redirect happens (should be near-instant)
    return (
        <div className="flex flex-col items-center justify-center min-h-[60vh]">
            <div className="relative">
                <div className="animate-spin w-16 h-16 border-4 border-kaspa-primary border-t-transparent rounded-full shadow-xl" />
                <div className="absolute inset-0 flex items-center justify-center">
                    <div className="w-8 h-8 bg-kaspa-primary rounded-full animate-ping opacity-25" />
                </div>
            </div>
            <h3 className="mt-8 text-xl font-bold tracking-tight">Verbindung wird hergestellt...</h3>
            <p className="mt-2 text-gray-500 text-sm animate-pulse">Sicherer Token-Austausch mit KaspaBattle Backend</p>
        </div>
    );
}
