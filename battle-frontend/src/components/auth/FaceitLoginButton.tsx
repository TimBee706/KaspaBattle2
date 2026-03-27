import { useState, useRef, useEffect } from 'react';
import { startFaceitLogin, startFaceitLink } from '../../api/auth';
import { useAuthStore } from '../../stores/useAuthStore';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

export function FaceitLoginButton() {
    const { isAuthenticated, user, isFaceitConnected, logout } = useAuthStore();
    const navigate = useNavigate();
    const { t } = useTranslation();
    const [dropdownOpen, setDropdownOpen] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    // Close dropdown on outside click
    useEffect(() => {
        function handleClickOutside(e: MouseEvent) {
            if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
                setDropdownOpen(false);
            }
        }
        if (dropdownOpen) {
            document.addEventListener('mousedown', handleClickOutside);
        }
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [dropdownOpen]);

    // Verbunden: Zeige Avatar + Dropdown
    if (isAuthenticated && user && isFaceitConnected) {
        const levelText = user.faceit_skill_level
            ? `LVL ${user.faceit_skill_level}`
            : null;
        const eloText = user.faceit_elo
            ? `${user.faceit_elo} ELO`
            : null;
        const subText = [levelText, eloText].filter(Boolean).join(' · ') || 'FACEIT';

        return (
            <div className="relative" ref={dropdownRef}>
                <div
                    className="flex items-center gap-3 bg-kaspa-card/50 border border-kaspa-border pr-1 pl-3 py-1 rounded-full shadow-md group cursor-pointer hover:border-kaspa-primary/50 transition-colors"
                    onClick={() => setDropdownOpen(!dropdownOpen)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => e.key === 'Enter' && setDropdownOpen(!dropdownOpen)}
                >
                    <div className="flex flex-col items-end">
                        <span className="text-[11px] text-white font-bold tracking-tight">{user.faceit_nickname}</span>
                        <span className="text-[9px] text-kaspa-primary uppercase tracking-widest leading-none">{subText}</span>
                    </div>

                    <div className="relative">
                        {user.faceit_avatar ? (
                            <img
                                src={user.faceit_avatar}
                                alt={user.faceit_nickname}
                                className="w-8 h-8 rounded-full border-2 border-kaspa-primary/50 group-hover:border-kaspa-primary transition-colors"
                            />
                        ) : (
                            <div
                                className="w-8 h-8 rounded-full bg-kaspa-border flex items-center justify-center text-xs font-bold text-kaspa-primary border-2 border-kaspa-primary/50"
                            >
                                {(user.faceit_nickname?.[0] || 'U').toUpperCase()}
                            </div>
                        )}
                        <div className="absolute -bottom-0 -right-0 w-2.5 h-2.5 bg-green-500 border-2 border-kaspa-dark rounded-full" />
                    </div>

                    {/* Dropdown arrow */}
                    <svg className={`w-3 h-3 text-gray-400 transition-transform ${dropdownOpen ? 'rotate-180' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                    </svg>
                </div>

                {/* Dropdown Menu */}
                {dropdownOpen && (
                    <div className="absolute right-0 top-full mt-2 w-56 bg-kaspa-card border border-kaspa-border rounded-xl shadow-2xl shadow-black/40 overflow-hidden z-50 animate-in fade-in slide-in-from-top-2 duration-200">
                        <div className="px-4 py-3 border-b border-kaspa-border bg-kaspa-dark/50">
                            <p className="text-xs text-gray-400 font-bold uppercase tracking-widest">FACEIT Account</p>
                            <p className="text-sm text-white font-bold truncate mt-0.5">{user.faceit_nickname}</p>
                        </div>

                        <div className="py-1">
                            <button
                                onClick={() => { setDropdownOpen(false); navigate('/profile'); }}
                                className="w-full text-left px-4 py-2.5 text-sm text-gray-300 hover:text-white hover:bg-kaspa-border/50 transition-colors flex items-center gap-3"
                            >
                                <span className="text-base">👤</span>
                                {t('navigation.profile')}
                            </button>

                            <a
                                href={`https://www.faceit.com/en/players/${user.faceit_nickname}`}
                                target="_blank"
                                rel="noopener noreferrer"
                                onClick={() => setDropdownOpen(false)}
                                className="w-full text-left px-4 py-2.5 text-sm text-gray-300 hover:text-white hover:bg-kaspa-border/50 transition-colors flex items-center gap-3"
                            >
                                <span className="text-base">🔗</span>
                                {t('profile.view_on_faceit')}
                            </a>
                        </div>

                        <div className="border-t border-kaspa-border py-1">
                            <button
                                onClick={() => { setDropdownOpen(false); navigate('/profile'); }}
                                className="w-full text-left px-4 py-2.5 text-sm text-orange-400 hover:text-orange-300 hover:bg-kaspa-border/50 transition-colors flex items-center gap-3"
                            >
                                <span className="text-base">⚡</span>
                                {t('profile.disconnect_faceit')}
                            </button>

                            <button
                                onClick={async () => {
                                    setDropdownOpen(false);
                                    await logout();
                                    navigate('/');
                                }}
                                className="w-full text-left px-4 py-2.5 text-sm text-red-400 hover:text-red-300 hover:bg-kaspa-border/50 transition-colors flex items-center gap-3"
                            >
                                <span className="text-base">🚪</span>
                                {t('profile.logout')}
                            </button>
                        </div>
                    </div>
                )}
            </div>
        );
    }

    // Nicht verbunden: Login-Button zeigen
    return (
        <button
            onClick={() => (isAuthenticated ? startFaceitLink() : startFaceitLogin())}
            className="flex items-center gap-2.5 px-5 py-2.5 bg-orange-600 hover:bg-orange-500 text-white rounded-xl text-sm font-bold transition-all shadow-lg shadow-orange-900/20 active:scale-95 group"
        >
            <svg className="w-5 h-5 group-hover:rotate-12 transition-transform" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93z" />
            </svg>
            Login with FACEIT
        </button>
    );
}
