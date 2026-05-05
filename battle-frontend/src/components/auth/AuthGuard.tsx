import { Link } from 'react-router-dom';
import { useAuthStore } from '../../stores/useAuthStore';
import { useWalletStore } from '../../stores/useWalletStore';

export function AuthGuard({ children }: { children: React.ReactNode }) {
    const { isAuthLoading, testMode, isFaceitConnected } = useAuthStore();
    const { isConnected: isWalletConnected } = useWalletStore();

    // Wait for initial auth check to complete before deciding
    if (isAuthLoading && !testMode) {
        return (
            <div className="flex items-center justify-center min-h-[60vh]">
                <div className="animate-spin w-8 h-8 border-2 border-kaspa-primary border-t-transparent rounded-full" />
            </div>
        );
    }

    const needsFaceit = !testMode && !isFaceitConnected;
    const needsWallet = !isWalletConnected;
    const showBanner = !testMode && (needsFaceit || needsWallet);

    return (
        <>
            {showBanner && <AuthBanner needsFaceit={needsFaceit} needsWallet={needsWallet} />}
            {children}
        </>
    );
}

function AuthBanner({ needsFaceit, needsWallet }: { needsFaceit: boolean; needsWallet: boolean }) {
    let message: string;

    if (needsFaceit && needsWallet) {
        message = 'Um an Matches teilzunehmen, musst du zuerst dein FaceIT-Konto und dein Kaspa Wallet verbinden.';
    } else if (needsWallet) {
        message = 'Um an Matches teilzunehmen, verbinde bitte noch dein Kaspa Wallet.';
    } else {
        message = 'Um an Matches teilzunehmen, verbinde bitte noch dein FaceIT-Konto.';
    }

    return (
        <div className="mb-6 p-4 bg-kaspa-card border border-amber-500/40 rounded-xl shadow-lg animate-in fade-in slide-in-from-top-2 duration-500">
            <div className="flex flex-col sm:flex-row items-start sm:items-center gap-4">
                <div className="flex items-center gap-3 flex-1 min-w-0">
                    <svg xmlns="http://www.w3.org/2000/svg" className="w-5 h-5 text-amber-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.75}><path strokeLinecap="round" strokeLinejoin="round" d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" /></svg>
                    <p className="text-sm text-gray-200 font-medium">{message}</p>
                </div>
                <div className="flex items-center gap-3 shrink-0">
                    {needsFaceit && (
                        <Link
                            to="/profile"
                            className="px-4 py-2 bg-kaspa-primary/10 border border-kaspa-primary/30 hover:bg-kaspa-primary/20 text-kaspa-primary rounded-lg text-xs font-bold uppercase tracking-wider transition-all whitespace-nowrap"
                        >
                            FaceIT verbinden
                        </Link>
                    )}
                    {needsWallet && (
                        <Link
                            to="/wallet/import"
                            className="px-4 py-2 bg-kaspa-primary/10 border border-kaspa-primary/30 hover:bg-kaspa-primary/20 text-kaspa-primary rounded-lg text-xs font-bold uppercase tracking-wider transition-all whitespace-nowrap"
                        >
                            Wallet verbinden
                        </Link>
                    )}
                </div>
            </div>
        </div>
    );
}
