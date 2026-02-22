import { Link } from 'react-router-dom';
import { WalletConnectButton } from '../wallet/WalletConnectButton';
import { FaceitLoginButton } from '../auth/FaceitLoginButton';

export function Header() {
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
                        <Link to="/create" className="text-gray-300 hover:text-white transition-colors">Challenge erstellen</Link>
                        <Link to="/history" className="text-gray-300 hover:text-white transition-colors">Verlauf</Link>
                    </nav>
                </div>

                <div className="flex items-center gap-4">
                    <WalletConnectButton />
                    <div className="w-px h-6 bg-kaspa-border hidden sm:block" />
                    <FaceitLoginButton />
                </div>
            </div>
        </header>
    );
}
