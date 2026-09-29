/**
 * Handles the query flags the backend appends after the server-side FACEIT callback:
 *   /lobby?linked=1        -> session cookie is set; reload /auth/me and /faceit/status
 *   /?error=<code>         -> login/link failed, nothing was stored
 *
 * Guarded by a module-level flag so React StrictMode's double effect run cannot fire it twice.
 * The OAuth code itself never reaches the SPA: the callback is redeemed server-side.
 */
export interface AuthRedirectDeps {
    fetchUser: () => Promise<void>;
    fetchFaceitStatus: () => Promise<unknown>;
    replaceUrl: (path: string) => void;
    notifyError: (code: string) => void;
}

export type AuthRedirectResult = 'linked' | 'error' | 'none';

let handled = false;

/** Test helper. */
export function resetAuthRedirectGuard(): void {
    handled = false;
}

export async function handleAuthRedirect(search: string, pathname: string, deps: AuthRedirectDeps): Promise<AuthRedirectResult> {
    const params = new URLSearchParams(search);
    const linked = params.get('linked');
    const error = params.get('error');
    if (!linked && !error) return 'none';
    if (handled) return 'none';
    handled = true;

    if (error) {
        deps.notifyError(error);
        deps.replaceUrl(pathname);
        return 'error';
    }

    try {
        // Server state is the source of truth: always reload, never trust persisted local auth.
        await deps.fetchUser();
        await deps.fetchFaceitStatus();
        deps.replaceUrl('/lobby');
        return 'linked';
    } catch {
        deps.notifyError('session_not_established');
        deps.replaceUrl('/');
        return 'error';
    }
}
