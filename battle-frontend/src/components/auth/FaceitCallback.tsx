import { useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';

/**
 * FACEIT OAuth Callback Handler
 *
 * FACEIT redirects to this frontend route after login:
 *   https://kaspabattle.com/auth/faceit/callback?code=...&state=...
 *
 * We simply forward code + state to the backend GET endpoint:
 *   /api/v1/faceit/callback?code=...&state=...
 *
 * The backend handles the full token exchange, creates the session,
 * sets the auth cookie via session-bounce, and redirects to /lobby.
 */
export function FaceitCallback() {
    const [searchParams] = useSearchParams();

    useEffect(() => {
        const code = searchParams.get('code');
        const state = searchParams.get('state');
        const errorParam = searchParams.get('error');

        if (errorParam) {
            console.error('❌ FACEIT OAuth error:', errorParam);
            window.location.replace(`/?error=${encodeURIComponent(errorParam)}`);
            return;
        }

        if (!code || !state) {
            console.error('❌ FACEIT callback: missing code or state');
            window.location.replace('/');
            return;
        }

        // Prevent React 18 StrictMode double-fire which invalidates the OAuth state
        const dedupeKey = `oauth_bounce_${state}`;
        if (sessionStorage.getItem(dedupeKey)) {
            return;
        }
        sessionStorage.setItem(dedupeKey, '1');

        // Forward to backend — backend handles token exchange + session-bounce
        const apiBase = import.meta.env.VITE_API_BASE_URL || '/api/v1';
        const backendCallback = `${apiBase}/faceit/callback?code=${encodeURIComponent(code)}&state=${encodeURIComponent(state)}`;
        console.log('🔄 FACEIT callback: forwarding to backend...', backendCallback);
        
        try {
            // FACEIT renders our redirect_uri inside an invisible iframe if not opened in a popup.
            // We MUST escape this iframe so the top-level browser window goes to our backend!
            if (window !== window.top) {
                console.log('⛓️ Detected FACEIT iframe jail. Breaking out...');
                window.top!.location.href = backendCallback;
                return;
            }
        } catch (e) {
            console.warn('⚠️ Frame-bust failed (CORS), falling back to frame-navigation', e);
        }

        window.location.replace(backendCallback);
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
