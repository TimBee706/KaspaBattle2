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
import { CreateTournamentPage } from './pages/CreateTournamentPage';
import { TournamentDetailPage } from './pages/TournamentDetailPage';
import { TournamentResultsPage } from './pages/TournamentResultsPage';
import { TournamentAdminPage } from './pages/TournamentAdminPage';
import { WhitepaperPage } from './pages/WhitepaperPage';
import { TermsPage } from './pages/TermsPage';
import { SupportPage } from './pages/SupportPage';
import { RegisterPage } from './pages/RegisterPage';
import { LoginPage } from './pages/LoginPage';
import { ForgotPasswordPage } from './pages/ForgotPasswordPage';
import { ResetPasswordPage } from './pages/ResetPasswordPage';
import { VerifyEmailPage } from './pages/VerifyEmailPage';
import { FreePlayLobbyPage } from './pages/FreePlayLobbyPage';
import { FreePlayGamePage } from './pages/FreePlayGamePage';
import { AccountPage } from './pages/AccountPage';

import { useAuthStore } from './stores/useAuthStore';
import { useWallet } from './hooks/useWallet';
import { handleAuthRedirect } from './auth/authRedirect';
import { faceitApi } from './api/faceit';

// Fallback for unknown routes goes to LandingPage

export default function App() {
  // Kaspa WASM warms up in the background. It must NOT gate the app: Free Play (and the whole
  // account area) work without it, and a broken SDK or node never blocks a free game.
  const { error: wasmError } = useKaspaInit();
  const { fetchUser } = useAuthStore();

  useWallet(); // Restore wallet session & balance on boot
  useBalance(); // Balance-Tracking im Hintergrund starten

  useEffect(() => {
    handleAuthRedirect(window.location.search, window.location.pathname, {
      fetchUser,
      fetchFaceitStatus: () => faceitApi.getStatus(),
      replaceUrl: (path) => window.history.replaceState({}, document.title, path),
      notifyError: (code) => alert(`Authentication Error: ${code}`),
    }).then((result) => {
      if (result === 'none' && !useAuthStore.getState().isAuthenticated) {
        // Cookie-based fallback: try to hydrate auth from HttpOnly cookie
        fetchUser().catch(() => {
          // Silently fail — user simply isn't logged in
        });
      }
    });
  }, [fetchUser]);

  useEffect(() => {
    if (wasmError) {
      console.warn('[App] Kaspa WASM SDK unavailable – on-chain features disabled, Free Play unaffected:', wasmError);
    }
  }, [wasmError]);

  return (
    <BrowserRouter>
      <ErrorBoundary>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<LandingPage />} />
            <Route path="auth/faceit/callback" element={<FaceitCallback />} />

            {/* Free Play + e-mail accounts: no wallet, no FACEIT, no Kaspa node needed */}
            <Route path="register" element={<RegisterPage />} />
            <Route path="login" element={<LoginPage />} />
            <Route path="forgot-password" element={<ForgotPasswordPage />} />
            <Route path="reset-password" element={<ResetPasswordPage />} />
            <Route path="verify-email" element={<VerifyEmailPage />} />
            <Route path="free-play" element={<FreePlayLobbyPage />} />
            <Route path="free-play/:id" element={<FreePlayGamePage />} />
            <Route path="account" element={<AccountPage />} />

            {/* Öffentliche Routen - sichtbar für alle */}
            <Route path="lobby" element={<LobbyPage />} />
            <Route path="lobby/create" element={<CreateMatchPage />} />
            <Route path="lobby/:lobbyId" element={<LobbyPage />} />
            <Route path="history" element={<HistoryPage />} />
            <Route path="tournaments" element={<TournamentListPage />} />
            <Route path="tournaments/create" element={<CreateTournamentPage />} />
            <Route path="tournaments/:id" element={<TournamentDetailPage />} />
            <Route path="tournaments/:id/results" element={<TournamentResultsPage />} />
            <Route path="admin/tournaments" element={<AuthGuard><TournamentAdminPage /></AuthGuard>} />
            <Route path="whitepaper" element={<WhitepaperPage />} />
            <Route path="terms" element={<TermsPage />} />
            <Route path="support" element={<SupportPage />} />

            {/* Geschützte Routen - AuthGuard zeigt Banner */}
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
