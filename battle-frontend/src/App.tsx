import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/layout/Layout';
import { AuthGuard } from './components/auth/AuthGuard';
import { FaceitCallback } from './components/auth/FaceitCallback';
import { useKaspaInit } from './hooks/useKaspaInit';
import { useBalance } from './hooks/useBalance';

// Pages - placeholder for now
const PlaceHolder = ({ title }: { title: string }) => (
  <div className="py-20 text-center">
    <h1 className="text-4xl font-black text-kaspa-primary mb-4 uppercase">{title}</h1>
    <p className="text-gray-500">Diese Seite wird gerade implementiert...</p>
  </div>
);

export default function App() {
  const { isReady, error: wasmError } = useKaspaInit();
  useBalance(); // Balance-Tracking im Hintergrund starten

  if (wasmError) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-kaspa-dark text-red-500 p-8 text-center">
        <div>
          <div className="text-5xl mb-6">⚠️</div>
          <h1 className="text-2xl font-bold mb-2">Kritischer Fehler</h1>
          <p className="font-mono text-sm opacity-70">WASM SDK: {wasmError}</p>
          <button
            onClick={() => window.location.reload()}
            className="mt-8 px-6 py-2 bg-kaspa-border hover:bg-gray-700 rounded-lg text-sm transition-colors"
          >
            Neu laden
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
          Kaspa WASM SDK wird geladen...
        </span>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<PlaceHolder title="Landing Page" />} />
          <Route path="auth/faceit/callback" element={<FaceitCallback />} />

          {/* Geschützte Routen */}
          <Route path="lobby" element={<AuthGuard><PlaceHolder title="Matchmaking Lobby" /></AuthGuard>} />
          <Route path="create" element={<AuthGuard><PlaceHolder title="Challenge erstellen" /></AuthGuard>} />
          <Route path="match/:matchId" element={<AuthGuard><PlaceHolder title="Live Match" /></AuthGuard>} />
          <Route path="history" element={<AuthGuard><PlaceHolder title="Match Verlauf" /></AuthGuard>} />
          <Route path="profile" element={<AuthGuard><PlaceHolder title="Profil" /></AuthGuard>} />

          <Route path="*" element={<PlaceHolder title="404 - Not Found" />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
