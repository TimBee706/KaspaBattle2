import { Link, useNavigate } from 'react-router-dom';
import { FaceitLoginButton } from '../auth/FaceitLoginButton';
import { useAuthStore } from '../../stores/useAuthStore';
import { useTranslation } from 'react-i18next';

export function Header() {
    const navigate = useNavigate();
    const { testMode, setTestMode } = useAuthStore();
    const { t } = useTranslation();

    return (
        <header className="bg-kaspa-card border-b border-kaspa-border sticky top-0 z-50">
            <div className="container mx-auto px-4 h-16 flex items-center justify-between">
                <div className="flex items-center gap-8">
                    <Link to="/" className="text-xl font-bold text-kaspa-primary tracking-tighter flex items-center gap-2">
                        <img src="/kaspa-battle_logo.svg" alt="Kaspa Battle Logo" className="h-8 w-auto" />
                        KASPABATTLE
                    </Link>

                    <nav className="hidden md:flex items-center gap-6">
                        <Link to="/lobby" className="text-gray-300 hover:text-white transition-colors">{t('navigation.lobby')}</Link>
                        <Link to="/lobby/create" className="text-gray-300 hover:text-white transition-colors">{t('navigation.create_challenge')}</Link>
                        <Link to="/history" className="text-gray-300 hover:text-white transition-colors">{t('navigation.history')}</Link>
                    </nav>
                </div>

                <div className="flex items-center gap-4">
                    {/* DEV Test Mode Toggle */}
                    {import.meta.env.MODE === 'development' && (
                        <div className="flex items-center gap-2 mr-2 bg-kaspa-dark/50 px-3 py-1.5 rounded-lg border border-kaspa-border">
                            <label className="text-xs text-gray-400 font-mono cursor-pointer" onClick={() => setTestMode(!testMode)}>
                                TestMode
                            </label>
                            <button
                                onClick={() => setTestMode(!testMode)}
                                className={`w-8 h-4 rounded-full relative transition-colors ${testMode ? 'bg-kaspa-primary' : 'bg-gray-600'}`}
                            >
                                <div className={`w-3 h-3 bg-white rounded-full absolute top-0.5 transition-transform ${testMode ? 'translate-x-4' : 'translate-x-0.5'}`} />
                            </button>
                        </div>
                    )}

                    <button
                        onClick={() => navigate('/wallet/import')}
                        className="px-6 py-2 bg-kaspa-primary/10 hover:bg-kaspa-primary/20 border border-kaspa-primary/30 text-kaspa-primary rounded-lg font-bold transition-all flex items-center gap-2"
                    >
                        {t('navigation.wallet')}
                    </button>
                    <div className="w-px h-6 bg-kaspa-border hidden sm:block" />
                    <FaceitLoginButton />
                </div>
            </div>
        </header>
    );
}
