import { Link, useNavigate } from 'react-router-dom';
import { FaceitLoginButton } from '../auth/FaceitLoginButton';

export function Header() {
    const navigate = useNavigate();

    return (
        <header className="bg-kaspa-card border-b border-kaspa-border sticky top-0 z-50">
            <div className="container mx-auto px-4 h-16 flex items-center justify-between">
                <div className="flex items-center gap-8">
                    <Link to="/" className="text-xl font-bold text-kaspa-primary tracking-tighter flex items-center gap-2">
                        <span className="text-2xl">⚡</span>
                        KASPABATTLE
                    </Link>

                    <nav className="hidden md:flex items-center gap-6">
                        <Link to="/lobby" className="text-gray-300 hover:text-white transition-colors">Lobby</Link>
                        <Link to="/lobby/create" className="text-gray-300 hover:text-white transition-colors">Challenge erstellen</Link>
                        <Link to="/history" className="text-gray-300 hover:text-white transition-colors">Verlauf</Link>
                    </nav>
                </div>

                <div className="flex items-center gap-4">
                    <button
                        onClick={() => navigate('/wallet/import')}
                        className="px-6 py-2 bg-kaspa-primary/10 hover:bg-kaspa-primary/20 border border-kaspa-primary/30 text-kaspa-primary rounded-lg font-bold transition-all flex items-center gap-2"
                    >
                        Wallet
                    </button>
                    <div className="w-px h-6 bg-kaspa-border hidden sm:block" />
                    <FaceitLoginButton />
                </div>
            </div>
        </header>
    );
}
