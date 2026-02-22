import { startFaceitLogin } from '../../api/auth';
import { useAuthStore } from '../../stores/useAuthStore';

export function FaceitLoginButton() {
    const { isAuthenticated, user, logout } = useAuthStore();

    if (isAuthenticated && user) {
        return (
            <div className="flex items-center gap-3 bg-kaspa-card/50 border border-kaspa-border pr-1 pl-3 py-1 rounded-full shadow-md group">
                <div className="flex flex-col items-end">
                    <span className="text-[11px] text-white font-bold tracking-tight">{user.faceit_nickname}</span>
                    <span className="text-[9px] text-gray-400 uppercase tracking-widest leading-none">Pro Player</span>
                </div>

                <div className="relative">
                    {user.faceit_avatar ? (
                        <img
                            src={user.faceit_avatar}
                            alt={user.faceit_nickname}
                            className="w-8 h-8 rounded-full border-2 border-kaspa-primary/50 group-hover:border-kaspa-primary transition-colors cursor-pointer"
                            onClick={logout}
                            title="Abmelden"
                        />
                    ) : (
                        <div
                            className="w-8 h-8 rounded-full bg-kaspa-border flex items-center justify-center text-xs font-bold text-kaspa-primary border-2 border-kaspa-primary/50 cursor-pointer"
                            onClick={logout}
                            title="Abmelden"
                        >
                            {user.faceit_nickname[0].toUpperCase()}
                        </div>
                    )}
                    <div className="absolute -bottom-0 -right-0 w-2.5 h-2.5 bg-green-500 border-2 border-kaspa-dark rounded-full" />
                </div>
            </div>
        );
    }

    return (
        <button
            onClick={startFaceitLogin}
            className="flex items-center gap-2.5 px-5 py-2.5 bg-orange-600 hover:bg-orange-500 text-white rounded-xl text-sm font-bold transition-all shadow-lg shadow-orange-900/20 active:scale-95 group"
        >
            <svg className="w-5 h-5 group-hover:rotate-12 transition-transform" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93z" />
            </svg>
            Login with FACEIT
        </button>
    );
}
