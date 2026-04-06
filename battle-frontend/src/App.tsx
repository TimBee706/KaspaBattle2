import { useEffect } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { ErrorBoundary } from './components/ErrorBoundary';
import { Layout } from './components/layout/Layout';
import { AuthGuard } from './components/auth/AuthGuard';
import { FaceitCallback } from './components/auth/FaceitCallback';
import { useKaspaInit } from './hooks/useKaspaInit';
import { useBalance } from './hooks/useBalance';
import { WalletPage } from './pages/WalletPage';
import { LandingPage } from './pages/LandingPage';
import { LobbyPage } from './pages/LobbyPage';
import { CreateMatchPage } from './pages/CreateMatchPage';
import { MatchPage } from './pages/MatchPage';
import { HistoryPage } from './pages/HistoryPage';
import { ProfilePage } from './pages/ProfilePage';
import { EscrowPage } from './pages/EscrowPage';
import { TournamentListPage } from './pages/TournamentListPage';
import { TournamentDetailPage } from './pages/TournamentDetailPage';
import { TournamentResultsPage } from './pages/TournamentResultsPage';
import { TournamentAdminPage } from './pages/TournamentAdminPage';

import { useAuthStore } from './stores/useAuthStore';
import { useTranslation } from 'react-i18next';
import { useWallet } from './hooks/useWallet';

// Fallback for unknown routes goes to LandingPage

export default function App() {
  const { isReady, error: wasmError } = useKaspaInit();
  const { fetchUser } = useAuthStore();
  const { t } = useTranslation();

  useWallet(); // Restore wallet session & balance on boot
  useBalance(); // Balance-Tracking im Hintergrund starten

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const linked = params.get('linked');
    const errorParam = params.get('error');

    if (linked) {
      console.log('\u{1F517} [App] ?linked=1 detected, calling fetchUser...');
      fetchUser()
        .then(() => {
          console.log('\u2705 [App] fetchUser succeeded after FaceIT login');
          // Clean querystring and navigate to lobby without a full page reload.
          // window.location.replace would lose the zustand in-memory state.
          window.history.replaceState({}, document.title, '/lobby');
        })
        .catch((err) => {
          console.error('\u274C [App] fetchUser failed after FaceIT login:', err);
          window.history.replaceState({}, document.title, '/');
        });
    } else if (errorParam) {
      console.error("FACEIT Auth Fehler:", errorParam);
      alert(`Authentication Error: ${errorParam}`);
      window.history.replaceState({}, document.title, window.location.pathname);
    } else if (!useAuthStore.getState().isAuthenticated) {
      // Cookie-based fallback: try to hydrate auth from HttpOnly cookie
      fetchUser().catch(() => {
        // Silently fail — user simply isn't logged in
      });
    }
  }, [fetchUser]);

  if (wasmError) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-kaspa-dark text-red-500 p-8 text-center">

        <div>
          <div className="text-5xl mb-6">{'\u26A0\uFE0F'}</div>
          <h1 className="text-2xl font-bold mb-2">{t('common.error_title')}</h1>
          <p className="font-mono text-sm opacity-70">WASM SDK: {wasmError}</p>
          <button
            onClick={() => window.location.reload()}
            className="mt-8 px-6 py-2 bg-kaspa-border hover:bg-gray-700 rounded-lg text-sm transition-colors"
          >
            {t('common.reload')}
          </button>
        </div>
      </div>
    );
  }

  if (!isReady) {
    return (
      <div className="flex flex-col items-center justify-center min-h-screen bg-kaspa-dark text-white">

        <div className="w-64 h-1.5 bg-kaspa-border rounded-full overflow-hidden mb-4 shadow-inner">
          <div className="h-full bg-kaspa-primary animate-[shimmer_2s_infinite] w-full origin-left" />
        </div>
        <span className="text-[10px] uppercase tracking-[0.3em] font-bold text-kaspa-primary animate-pulse">
          {t('common.loading_sdk')}
        </span>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <ErrorBoundary>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<LandingPage />} />
            <Route path="auth/faceit/callback" element={<FaceitCallback />} />

            {/* Oeffentliche Routen - sichtbar fuer alle */}
            <Route path="lobby" element={<LobbyPage />} />
            <Route path="lobby/create" element={<CreateMatchPage />} />
            <Route path="lobby/:lobbyId" element={<LobbyPage />} />
            <Route path="history" element={<HistoryPage />} />
            <Route path="tournaments" element={<TournamentListPage />} />
            <Route path="tournaments/:id" element={<TournamentDetailPage />} />
            <Route path="tournaments/:id/results" element={<TournamentResultsPage />} />
            <Route path="admin/tournaments" element={<AuthGuard><TournamentAdminPage /></AuthGuard>} />

            {/* Geschuetzte Routen - AuthGuard zeigt Banner */}
            <Route path="escrow/:lobbyId" element={<AuthGuard><EscrowPage /></AuthGuard>} />
            <Route path="match/:matchId" element={<AuthGuard><MatchPage /></AuthGuard>} />
            <Route path="profile" element={<AuthGuard><ProfilePage /></AuthGuard>} />
            <Route path="wallet/import" element={<WalletPage />} />

            <Route path="*" element={<LandingPage />} />
          </Route>
        </Routes>
      </ErrorBoundary>
    </BrowserRouter>
  );
}
