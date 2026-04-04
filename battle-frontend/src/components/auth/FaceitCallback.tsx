import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { handleFaceitCallback } from '../../api/auth';
import { useAuthStore } from '../../stores/useAuthStore';

export function FaceitCallback() {
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const { setAuth } = useAuthStore();
    const code = searchParams.get('code');
    const state = searchParams.get('state');
    const errorParam = searchParams.get('error');
    const [error, setError] = useState<string | null>(
        errorParam
            ? `FACEIT Login fehlgeschlagen: ${errorParam}`
            : !code || !state
                ? 'Ungueltiger Callback - fehlende Parameter'
                : null,
    );

    useEffect(() => {
        if (errorParam || !code || !state) {
            return;
        }

        handleFaceitCallback(code, state)
            .then((response) => {
                setAuth(response.user, response.tokens);
                navigate('/lobby', { replace: true });
            })
            .catch((err) => {
                setError(err instanceof Error ? err.message : 'FACEIT Login fehlgeschlagen');
            });
    }, [code, errorParam, navigate, setAuth, state]);

    if (error) {
        return (
            <div className="flex flex-col items-center justify-center min-h-[60vh] text-center px-4">
                <div className="w-16 h-16 bg-red-900/20 text-red-500 rounded-full flex items-center justify-center text-3xl mb-6">x</div>
                <h2 className="text-2xl font-bold mb-2">Authentifizierung fehlgeschlagen</h2>
                <p className="text-gray-400 max-w-md mb-8">{error}</p>
                <button
                    onClick={() => navigate('/')}
                    className="px-6 py-3 bg-kaspa-primary text-kaspa-dark font-bold rounded-xl hover:bg-kaspa-secondary transition-colors"
                >
                    Zurueck zur Startseite
                </button>
            </div>
        );
    }

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
