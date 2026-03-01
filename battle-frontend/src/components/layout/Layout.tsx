import { Outlet } from 'react-router-dom';
import { Header } from './Header';
import { Footer } from './Footer';
import { LanguageToggle } from '../LanguageToggle';

export function Layout() {
    return (
        <div className="min-h-screen bg-kaspa-dark text-white flex flex-col">
            <LanguageToggle />
            <Header />
            <main className="flex-1 container mx-auto px-4 py-8">
                <Outlet />
            </main>
            <Footer />
        </div>
    );
}
