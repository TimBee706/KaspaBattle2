import { useEffect, useState } from 'react';
import { useAuthStore } from '../stores/useAuthStore';
import { formatKas, shortenAddress } from '../utils/format';
import { faceitApi } from '../api/faceit';
import type { FaceitProfileResponse } from '../api/types';
import type { FaceitStatsResponse } from '../api/faceit';
import { useTranslation } from 'react-i18next';
import { startFaceitLogin } from '../api/auth';
import { useNavigate } from 'react-router-dom';

// ── Loading skeleton ──────────────────────────────────────────────────────
function Skeleton({ className = '' }: { className?: string }) {
    return <div className={`animate-pulse bg-slate-700/50 rounded ${className}`} />;
}

function StatSkeleton() {
    return (
        <div className="card text-center p-8">
            <Skeleton className="h-3 w-20 mx-auto mb-4" />
            <Skeleton className="h-10 w-16 mx-auto" />
        </div>
    );
}

export function ProfilePage() {
    const { user, isFaceitConnected, fetchUser } = useAuthStore();
    const [faceitData, setFaceitData] = useState<FaceitProfileResponse | null>(null);
    const [gameStats, setGameStats] = useState<FaceitStatsResponse | null>(null);
    const [loadingProfile, setLoadingProfile] = useState(true);
    const [loadingStats, setLoadingStats] = useState(true);
    const [statsError, setStatsError] = useState<string | null>(null);
    const [disconnecting, setDisconnecting] = useState(false);
    const [showDisconnectConfirm, setShowDisconnectConfirm] = useState(false);
    const { t } = useTranslation();
    const navigate = useNavigate();

    // Load FACEIT profile
    useEffect(() => {
        if (!user || !isFaceitConnected) { setLoadingProfile(false); return; }
        setLoadingProfile(true);
        faceitApi.getProfile()
            .then(res => { setFaceitData(res); setLoadingProfile(false); })
            .catch(err => {
                console.error("Fehler beim Abrufen des FACEIT Profils:", err);
                setLoadingProfile(false);
            });
    }, [user, isFaceitConnected]);

    // Load FACEIT game stats
    useEffect(() => {
        if (!user || !isFaceitConnected) { setLoadingStats(false); return; }
        setLoadingStats(true);
        setStatsError(null);
        faceitApi.getStats('cs2')
            .then(res => { setGameStats(res); setLoadingStats(false); })
            .catch(err => {
                console.error("Fehler beim Abrufen der FACEIT Stats:", err);
                setStatsError("Spiel-Stats konnten nicht geladen werden.");
                setLoadingStats(false);
            });
    }, [user, isFaceitConnected]);

    const handleDisconnect = async () => {
        setDisconnecting(true);
        try {
            await faceitApi.disconnect();
            await fetchUser();
            setShowDisconnectConfirm(false);
            navigate('/lobby');
        } catch (err) {
            console.error("FACEIT Disconnect Fehler:", err);
        } finally {
            setDisconnecting(false);
        }
    };

    if (!user) return null;

    // Not connected → Prompt to connect
    if (!isFaceitConnected) {
        return (
            <div className="flex flex-col items-center justify-center min-h-[60vh] text-center px-4">
                <div className="text-5xl mb-6">🔗</div>
                <h2 className="text-2xl font-bold mb-2">FACEIT nicht verbunden</h2>
                <p className="text-gray-400 max-w-md mb-8">Um dein Profil zu sehen, verbinde zuerst deinen FACEIT-Account.</p>
                <button
                    onClick={startFaceitLogin}
                    className="px-6 py-3 bg-orange-600 hover:bg-orange-500 text-white font-bold rounded-xl transition-all"
                >
                    Mit FACEIT verbinden
                </button>
            </div>
        );
    }

    const displayName = user.faceit_nickname || user.display_name || 'User';
    const winRate = user.total_matches > 0 ? (user.wins / user.total_matches) * 100 : 0;

    // Parse game stats lifetime data
    const lifetime = gameStats?.lifetime?.lifetime || gameStats?.lifetime || null;
    const csMatches = lifetime?.Matches || lifetime?.matches || '—';
    const csWinRate = lifetime?.['Win Rate %'] || lifetime?.win_rate || '—';
    const csKD = lifetime?.['Average K/D Ratio'] || lifetime?.average_kd || '—';
    const csHS = lifetime?.['Average Headshots %'] || lifetime?.average_headshots || '—';
    const csWins = lifetime?.Wins || lifetime?.wins || '—';
    const csRecentResults = lifetime?.['Recent Results'] || lifetime?.recent_results || [];

    return (
        <div className="max-w-4xl mx-auto space-y-8 py-8 px-4 animate-in fade-in slide-in-from-bottom-4 duration-500">

            {/* ── Profile Header ───────────────────────────────────── */}
            <div className="card flex flex-col md:flex-row items-center gap-8 p-10 bg-gradient-to-br from-kaspa-card to-kaspa-dark">
                <div className="relative">
                    {loadingProfile ? (
                        <Skeleton className="w-32 h-32 rounded-full" />
                    ) : user.faceit_avatar ? (
                        <img src={user.faceit_avatar} alt="" className="w-32 h-32 rounded-full border-4 border-kaspa-primary shadow-2xl" />
                    ) : (
                        <div className="w-32 h-32 rounded-full bg-kaspa-border flex items-center justify-center text-5xl font-black border-4 border-kaspa-primary text-kaspa-primary">
                            {displayName[0]?.toUpperCase() || 'U'}
                        </div>
                    )}
                    <div className="absolute -bottom-2 left-1/2 -translate-x-1/2 px-4 py-1 bg-kaspa-primary text-kaspa-dark text-[10px] font-black rounded-full uppercase tracking-tighter shadow-lg shadow-kaspa-primary/20">
                        {faceitData && faceitData.skill_level > 0
                            ? `LVL ${faceitData.skill_level}`
                            : (user.faceit_skill_level ? `LVL ${user.faceit_skill_level}` : t('profile.level_unknown'))}
                    </div>
                </div>

                <div className="flex-1 text-center md:text-left">
                    <h1 className="text-4xl font-black tracking-tighter mb-1">{displayName}</h1>
                    <p className="text-gray-500 font-mono text-sm mb-6">{shortenAddress(user.kaspa_address)}</p>

                    <div className="flex flex-wrap justify-center md:justify-start gap-3">
                        <div className="px-3 py-1 bg-kaspa-border rounded border border-gray-700 text-xs font-bold text-gray-400">
                            ID: {user.faceit_id}
                        </div>
                        <div className="px-3 py-1 bg-orange-600/20 rounded border border-orange-500/30 text-xs font-bold text-orange-400">
                            {t('profile.verified')}
                        </div>
                        {(faceitData?.elo || user.faceit_elo) ? (
                            <div className="px-3 py-1 bg-kaspa-primary/10 rounded border border-kaspa-primary/30 text-xs font-bold text-kaspa-primary">
                                {faceitData?.elo || user.faceit_elo} ELO
                                {faceitData?.is_cached && <span className="text-gray-500 font-normal ml-1">(cached)</span>}
                            </div>
                        ) : null}
                        {faceitData?.faceit_url && (
                            <a
                                href={faceitData.faceit_url}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="px-3 py-1 bg-slate-700/50 rounded border border-slate-600 text-xs font-bold text-gray-300 hover:text-white hover:border-gray-500 transition-colors"
                            >
                                Auf FACEIT anzeigen ↗
                            </a>
                        )}
                    </div>
                </div>
            </div>

            {/* ── FACEIT Game Stats (CS2) ─────────────────────────── */}
            <div>
                <h3 className="text-sm font-black text-gray-500 uppercase tracking-widest mb-6 border-l-4 border-orange-500 pl-4 flex items-center gap-3">
                    CS2 FACEIT Stats
                    {gameStats?.is_cached && <span className="text-[9px] bg-yellow-500/10 text-yellow-400 border border-yellow-500/30 px-2 py-0.5 rounded-full font-bold normal-case">cached</span>}
                </h3>

                {statsError ? (
                    <div className="card p-6 text-center">
                        <p className="text-red-400 text-sm mb-4">{statsError}</p>
                        <button
                            onClick={() => {
                                setStatsError(null);
                                setLoadingStats(true);
                                faceitApi.getStats('cs2')
                                    .then(res => { setGameStats(res); setLoadingStats(false); })
                                    .catch(() => { setStatsError("Erneuter Versuch fehlgeschlagen."); setLoadingStats(false); });
                            }}
                            className="px-4 py-2 bg-kaspa-border hover:bg-gray-700 rounded-lg text-sm font-bold transition-colors"
                        >
                            Erneut versuchen
                        </button>
                    </div>
                ) : loadingStats ? (
                    <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
                        {[...Array(5)].map((_, i) => <StatSkeleton key={i} />)}
                    </div>
                ) : (
                    <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
                        <div className="card text-center p-6">
                            <span className="text-[9px] font-black text-gray-500 uppercase tracking-widest block mb-2">Matches</span>
                            <span className="text-3xl font-black text-white">{csMatches}</span>
                        </div>
                        <div className="card text-center p-6 border-b-4 border-b-green-500">
                            <span className="text-[9px] font-black text-gray-500 uppercase tracking-widest block mb-2">Win Rate</span>
                            <span className="text-3xl font-black text-green-400">{csWinRate}%</span>
                        </div>
                        <div className="card text-center p-6 border-b-4 border-b-blue-500">
                            <span className="text-[9px] font-black text-gray-500 uppercase tracking-widest block mb-2">K/D</span>
                            <span className="text-3xl font-black text-blue-400">{csKD}</span>
                        </div>
                        <div className="card text-center p-6 border-b-4 border-b-yellow-500">
                            <span className="text-[9px] font-black text-gray-500 uppercase tracking-widest block mb-2">HS %</span>
                            <span className="text-3xl font-black text-yellow-400">{csHS}%</span>
                        </div>
                        <div className="card text-center p-6 border-b-4 border-b-emerald-500">
                            <span className="text-[9px] font-black text-gray-500 uppercase tracking-widest block mb-2">Wins</span>
                            <span className="text-3xl font-black text-emerald-400">{csWins}</span>
                        </div>
                    </div>
                )}

                {/* Recent Results Streak */}
                {Array.isArray(csRecentResults) && csRecentResults.length > 0 && (
                    <div className="mt-4 flex items-center gap-2">
                        <span className="text-[9px] font-black text-gray-500 uppercase tracking-widest">Letzte Ergebnisse:</span>
                        <div className="flex gap-1">
                            {csRecentResults.slice(0, 20).map((r: string, i: number) => (
                                <div
                                    key={i}
                                    className={`w-5 h-5 rounded text-[9px] font-black flex items-center justify-center ${
                                        r === '1' || r.toLowerCase() === 'w'
                                            ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                                            : 'bg-red-500/20 text-red-400 border border-red-500/30'
                                    }`}
                                >
                                    {r === '1' || r.toLowerCase() === 'w' ? 'W' : 'L'}
                                </div>
                            ))}
                        </div>
                    </div>
                )}
            </div>

            {/* ── KaspaBattle Stats Grid ──────────────────────────── */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
                <div className="card text-center p-8">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">{t('profile.stats.total_matches')}</span>
                    <span className="text-4xl font-black text-white">{user.total_matches}</span>
                </div>
                <div className="card text-center p-8 border-b-4 border-b-green-500">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">{t('profile.stats.wins')}</span>
                    <span className="text-4xl font-black text-green-500">{user.wins}</span>
                </div>
                <div className="card text-center p-8 border-b-4 border-b-red-500">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">{t('profile.stats.losses')}</span>
                    <span className="text-4xl font-black text-red-500">{user.losses}</span>
                </div>
                <div className="card text-center p-8 border-b-4 border-b-kaspa-primary">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">{t('profile.stats.win_rate')}</span>
                    <span className="text-4xl font-black text-kaspa-primary">{winRate.toFixed(1)}%</span>
                </div>
            </div>

            {/* ── Financials ──────────────────────────────────────── */}
            <div className="card grid grid-cols-1 md:grid-cols-2 gap-8 p-10">
                <div>
                    <h3 className="text-sm font-black text-gray-500 uppercase tracking-widest mb-6 border-l-4 border-kaspa-primary pl-4">{t('profile.financials.title')}</h3>
                    <div className="space-y-4">
                        <div className="flex justify-between items-center">
                            <span className="text-gray-400 text-sm">{t('profile.financials.total_wagered')}</span>
                            <span className="font-bold text-white">{formatKas(user.total_wagered_sompi)} KAS</span>
                        </div>
                        <div className="flex justify-between items-center">
                            <span className="text-gray-400 text-sm">{t('profile.financials.total_won')}</span>
                            <span className="font-bold text-kaspa-primary">+{formatKas(user.total_won_sompi)} KAS</span>
                        </div>
                        <div className="h-px bg-kaspa-border" />
                        <div className="flex justify-between items-center">
                            <span className="text-gray-400 text-sm font-bold">{t('profile.financials.net_profit')}</span>
                            <span className="text-lg font-black text-emerald-500">+{(formatKas(user.total_won_sompi - (user.total_wagered_sompi / 2)))} KAS</span>
                        </div>
                    </div>
                </div>

                <div className="flex items-center justify-center p-6 bg-kaspa-dark rounded-2xl border border-kaspa-border border-dashed">
                    <div className="text-center">
                        <p className="text-[10px] text-gray-500 font-black uppercase mb-2">{t('profile.rank')}</p>
                        <div className="text-6xl mb-2">🥈</div>
                        <p className="text-xl font-bold italic tracking-tighter">SILVER COMMANDER</p>
                        <p className="text-[10px] text-gray-600 mt-2">{t('profile.next_level', { count: 10 })}</p>
                    </div>
                </div>
            </div>

            {/* ── FACEIT Disconnect ───────────────────────────────── */}
            <div className="card p-6 border border-red-500/10">
                <div className="flex items-center justify-between">
                    <div>
                        <h4 className="text-sm font-bold text-white mb-1">FACEIT-Verbindung</h4>
                        <p className="text-xs text-gray-500">Verbunden als <strong className="text-gray-300">{user.faceit_nickname}</strong>. Das Trennen beendet nicht deine KaspaBattle-Session.</p>
                    </div>
                    {showDisconnectConfirm ? (
                        <div className="flex gap-2">
                            <button
                                onClick={handleDisconnect}
                                disabled={disconnecting}
                                className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-xs font-bold rounded-lg transition-all disabled:opacity-50"
                            >
                                {disconnecting ? 'Trenne...' : 'Ja, trennen'}
                            </button>
                            <button
                                onClick={() => setShowDisconnectConfirm(false)}
                                className="px-4 py-2 bg-kaspa-border hover:bg-gray-700 text-white text-xs font-bold rounded-lg transition-all"
                            >
                                Abbrechen
                            </button>
                        </div>
                    ) : (
                        <button
                            onClick={() => setShowDisconnectConfirm(true)}
                            className="px-4 py-2 bg-red-600/10 hover:bg-red-600/20 text-red-400 text-xs font-bold rounded-lg border border-red-500/20 transition-all"
                        >
                            FACEIT trennen
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
