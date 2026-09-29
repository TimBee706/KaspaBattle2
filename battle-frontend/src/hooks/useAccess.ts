import { useAuthStore } from '../stores/useAuthStore';
import { deriveAccess, type AccessState } from '../domain/access';

/**
 * Single source for "may this user play?" decisions in the UI
 * (isAuthenticated / hasWallet / hasFaceit / canPlayNative / canPlayFaceit).
 */
export function useAccess(): AccessState {
    const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
    const walletConnected = useAuthStore((s) => s.walletConnected);
    const isFaceitConnected = useAuthStore((s) => s.isFaceitConnected);
    const testMode = useAuthStore((s) => s.testMode);
    return deriveAccess({ isAuthenticated, walletConnected, isFaceitConnected, testMode });
}
