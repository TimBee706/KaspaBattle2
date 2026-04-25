import React, { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { FaceitLoginButton } from '../auth/FaceitLoginButton';
import { useAuthStore } from '../../stores/useAuthStore';
import { useTranslation } from 'react-i18next';
import { LanguageToggle } from '../LanguageToggle';

export function Header() {
    const navigate = useNavigate();
    const { testMode, setTestMode } = useAuthStore();
    const { t } = useTranslation();
    const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);

    const closeMenu = () => setIsMobileMenuOpen(false);

    return (
        <header className="bg-kaspa-card border-b border-kaspa-border sticky top-0 z-50">
            <div className="container mx-auto px-4 h-16 flex items-center justify-between">
                <div className="flex items-center gap-8">
                    <Link to="/" className="text-xl font-bold text-kaspa-primary tracking-tighter flex items-center gap-2" onClick={closeMenu}>
                        <img src="/KaspaBattleLogo.svg" alt="Kaspa Battle Logo" className="h-8 w-auto" />
                        <span className="hidden sm:inline">KASPABATTLE</span>
                    </Link>

                    {/* Desktop Navigation */}
                    <nav className="hidden md:flex items-center gap-6">
                        <Link to="/lobby" className="text-gray-300 hover:text-white transition-colors">{t('navigation.lobby')}</Link>
                        {testMode && (
                            <Link to="/tournaments" className="text-gray-300 hover:text-[#49EACB] transition-colors">{t('tournaments.title')}</Link>
                        )}
                        <Link to="/lobby/create" className="text-gray-300 hover:text-white transition-colors">{t('navigation.create_challenge')}</Link>
                        <Link to="/history" className="text-gray-300 hover:text-white transition-colors">{t('navigation.history')}</Link>
                    </nav>
                </div>

                {/* Desktop Right Side */}
                <div className="hidden md:flex items-center gap-4">
                    {/* DEV Test Mode Toggle */}
                    {import.meta.env.VITE_ENABLE_TEST_MODE === 'true' && (
                        <div className="flex items-center gap-2 mr-2 bg-kaspa-dark/50 px-3 py-1.5 rounded-lg border border-kaspa-border">
                            <label className="text-xs text-gray-400 font-mono cursor-pointer" onClick={() => setTestMode(!testMode)}>
                                {t('common.header.test_mode')}
                            </label>
                            <button
                                onClick={() => setTestMode(!testMode)}
                                className={`w-8 h-4 rounded-full relative transition-colors ${testMode ? 'bg-kaspa-primary' : 'bg-gray-600'}`}
                            >
                                <div className={`w-3 h-3 bg-white rounded-full absolute top-0.5 transition-transform ${testMode ? 'translate-x-4' : 'translate-x-0.5'}`} />
                            </button>
                        </div>
                    )}

                    <LanguageToggle />

                    <button
                        onClick={() => navigate('/wallet/import')}
                        className="px-6 py-2 bg-kaspa-primary/10 hover:bg-kaspa-primary/20 border border-kaspa-primary/30 text-kaspa-primary rounded-lg font-bold transition-all flex items-center gap-2"
                    >
                        {t('navigation.wallet')}
                    </button>
                    <div className="w-px h-6 bg-kaspa-border hidden sm:block" />
                    <FaceitLoginButton />
                </div>

                {/* Mobile Menu Toggle Button */}
                <div className="md:hidden flex items-center gap-3">
                    <LanguageToggle />
                    <button 
                        onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
                        className="p-2 text-kaspa-primary hover:bg-kaspa-border rounded-lg transition-colors"
                        aria-label="Toggle menu"
                    >
                        <svg className="w-6 h-6 outline-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            {isMobileMenuOpen ? (
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                            ) : (
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                            )}
                        </svg>
                    </button>
                </div>
            </div>

            {/* Mobile Navigation Dropdown */}
            {isMobileMenuOpen && (
                <div className="md:hidden bg-kaspa-card border-b border-kaspa-border absolute w-full left-0 top-16 shadow-2xl">
                    <div className="flex flex-col p-4 gap-4">
                        <nav className="flex flex-col gap-4 pb-4 border-b border-kaspa-border/50">
                            <Link to="/lobby" onClick={closeMenu} className="text-gray-300 hover:text-white font-bold">{t('navigation.lobby')}</Link>
                            <Link to="/lobby/create" onClick={closeMenu} className="text-gray-300 hover:text-white font-bold">{t('navigation.create_challenge')}</Link>
                            <Link to="/history" onClick={closeMenu} className="text-gray-300 hover:text-white font-bold">{t('navigation.history')}</Link>
                        </nav>
                        
                        {import.meta.env.VITE_ENABLE_TEST_MODE === 'true' && (
                            <div className="flex justify-between items-center bg-kaspa-dark/50 p-3 rounded-lg border border-kaspa-border">
                                <span className="text-sm text-gray-400 font-mono">{t('common.header.test_mode')}</span>
                                <button
                                    onClick={() => setTestMode(!testMode)}
                                    className={`w-10 h-5 rounded-full relative transition-colors ${testMode ? 'bg-kaspa-primary' : 'bg-gray-600'}`}
                                >
                                    <div className={`w-4 h-4 bg-white rounded-full absolute top-0.5 transition-transform ${testMode ? 'translate-x-5' : 'translate-x-1'}`} />
                                </button>
                            </div>
                        )}
                        
                        <div className="flex flex-col gap-3 pt-2">
                            <button
                                onClick={() => { closeMenu(); navigate('/wallet/import'); }}
                                className="w-full py-3 bg-kaspa-primary/10 hover:bg-kaspa-primary/20 border border-kaspa-primary/30 text-kaspa-primary rounded-lg font-bold transition-all justify-center"
                            >
                                {t('navigation.wallet')}
                            </button>
                            <div className="w-full flex justify-center">
                                <FaceitLoginButton />
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </header>
    );
}
