import { useState, useEffect, useCallback } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  listTournaments,
  createTournament,
  sompiToKas,
  STATUS_LABELS,
  STATUS_COLORS,
  type Tournament,
  type TournamentStatus,
} from '../api/tournaments';
import { startFaceitLogin } from '../api/auth';
import { useAuthStore } from '../stores/useAuthStore';
import { useTranslation } from 'react-i18next';

// ─── Status Badge ─────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: TournamentStatus }) {
  const color = STATUS_COLORS[status] ?? '#6b7280';
  const label = STATUS_LABELS[status] ?? status;
  return (
    <span
      style={{ color, borderColor: color + '44', backgroundColor: color + '18' }}
      className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-semibold border"
    >
      <span
        style={{ backgroundColor: color }}
        className="w-1.5 h-1.5 rounded-full animate-pulse"
      />
      {label}
    </span>
  );
}



// ─── Tournament Card ──────────────────────────────────────────────────────────

function TournamentCard({ tournament }: { tournament: Tournament }) {
  const { t } = useTranslation();
  const kas = sompiToKas(tournament.buy_in_sompi);
  const isFinalPool = ['COMPLETED', 'BRACKET_READY', 'IN_PROGRESS', 'FUNDED'].includes(tournament.status);
  const poolSompi = isFinalPool ? tournament.total_prize_pool_sompi : (tournament.buy_in_sompi * tournament.max_teams);
  const pool = sompiToKas(poolSompi);

  return (
    <Link
      to={`/tournaments/${tournament.id}`}
      id={`tournament-card-${tournament.id}`}
      className="group bg-slate-900/60 border border-slate-700/50 p-4 rounded-xl flex flex-col md:flex-row justify-between items-start md:items-center gap-4 hover:border-kaspa-primary/40 hover:bg-slate-800/80 transition-all cursor-pointer active:scale-[0.99]"
    >
      {/* Left side */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-3 mb-1">
          <h3 className="font-bold text-white text-lg truncate group-hover:text-kaspa-primary transition-colors">
            {tournament.name}
          </h3>
          <StatusBadge status={tournament.status} />
        </div>
        <div className="flex items-center gap-3 text-xs text-slate-500 font-bold">
          <span className="uppercase tracking-wider">{tournament.game_id.toUpperCase()}</span>
          <span>•</span>
          <span>{tournament.max_teams} Teams</span>
          {tournament.registration_deadline && (
            <>
              <span>•</span>
              <span>{t('tournaments.card.deadline')}: {new Date(tournament.registration_deadline).toLocaleString()}</span>
            </>
          )}
        </div>
      </div>

      {/* Right side */}
      <div className="text-right shrink-0">
        <div className="text-emerald-400 text-xl font-black flex items-baseline gap-1 justify-end">
          {kas} <span className="text-sm font-bold text-emerald-400/80">KAS</span>
        </div>
        <div className="text-xs text-slate-500 font-bold uppercase tracking-widest mt-0.5">
          {t('tournaments.card.prize_pool')} {pool} KAS
        </div>
      </div>
    </Link>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export function TournamentListPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [tournaments, setTournaments] = useState<Tournament[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<TournamentStatus | 'ALL'>('ALL');
  const user = useAuthStore(s => s.user);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listTournaments();
      setTournaments(data);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleCreateClick = async () => {
    if (user) {
      navigate('/tournaments/create');
      return;
    }

    try {
      await startFaceitLogin();
    } catch {
      // ignore
    }
  };

  const filtered = filter === 'ALL' ? tournaments : tournaments.filter(t => t.status === filter);
  const filterOptions: Array<{ value: TournamentStatus | 'ALL'; label: string }> = [
    { value: 'ALL', label: t('tournaments.filter.all') },
    { value: 'REGISTRATION', label: t('tournaments.filter.open') },
    { value: 'IN_PROGRESS', label: t('tournaments.filter.live') },
    { value: 'COMPLETED', label: t('tournaments.filter.completed') },
  ];

  return (
    <div className="container mx-auto px-4 py-8">
      {/* Header – mirrors LobbyPage */}
      <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
        <div>
          <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">
            {t('tournaments.title')}
          </h1>
          <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">
            {t('tournaments.subtitle')}
          </p>
        </div>
        <div className="flex items-center gap-3 w-full md:w-auto">
          <button
            id="refresh-tournaments-btn"
            onClick={load}
            className="p-2 rounded-lg border border-white/10 hover:border-kaspa-primary/30 text-gray-400 hover:text-kaspa-primary transition-all shrink-0"
            title={t('tournaments.refresh')}
          >
            ↻
          </button>
          <button
            id="create-tournament-btn"
            onClick={() => { void handleCreateClick(); }}
            className="w-full md:w-auto bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-8 py-3 rounded-xl font-black uppercase tracking-tighter transition-all shadow-xl shadow-kaspa-primary/10 active:scale-95 text-center"
          >
            {user ? t('tournaments.create_btn') : t('tournaments.login_create_btn')}
          </button>
        </div>
      </div>

      {/* Filter pills */}
      <div className="flex gap-2 mb-6 flex-wrap">
        {filterOptions.map(opt => (
          <button
            key={opt.value}
            id={`filter-${opt.value.toLowerCase()}`}
            onClick={() => setFilter(opt.value)}
            className={`px-4 py-1.5 rounded-full text-sm font-bold uppercase tracking-widest transition-all duration-200 ${
              filter === opt.value
                ? 'bg-kaspa-primary text-kaspa-dark'
                : 'bg-white/5 text-slate-500 hover:text-white hover:bg-white/10'
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>

      <div className="mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary">
        {/* Content */}
        {loading ? (
          <div className="flex flex-col gap-3">
            {[...Array(5)].map((_, i) => (
              <div key={i} className="bg-slate-800/40 border border-slate-700/50 rounded-xl p-5 animate-pulse h-20" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <div className="text-center py-24 text-gray-500">
            <svg width="56" height="56" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg" className="mx-auto mb-4 text-gray-700">
              <path d="M18 7H38V26C38 32.627 32.627 38 26 38C19.373 38 14 32.627 14 26V7H18Z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round"/>
              <path d="M18 11H10C10 11 7 16 10 22C11.3 24.5 14 26 18 26" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" fill="none"/>
              <path d="M38 11H46C46 11 49 16 46 22C44.7 24.5 42 26 38 26" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" fill="none"/>
              <rect x="24" y="38" width="8" height="6" stroke="currentColor" strokeWidth="1.5" fill="none"/>
              <rect x="18" y="44" width="20" height="5" rx="2" stroke="currentColor" strokeWidth="1.5" fill="none"/>
            </svg>
            <p className="text-lg font-semibold text-gray-400">{t('tournaments.empty.title')}</p>
            <p className="text-sm mt-1">
              {user ? t('tournaments.empty.subtitle_auth') : t('tournaments.empty.subtitle_guest')}
            </p>
            <button
              id="empty-state-create-tournament-btn"
              onClick={() => { void handleCreateClick(); }}
              className="mt-6 w-full md:w-auto bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-8 py-3 rounded-xl font-black uppercase tracking-tighter transition-all shadow-xl shadow-kaspa-primary/10 active:scale-95"
            >
              {user ? t('tournaments.create_btn') : t('tournaments.login_create_btn')}
            </button>
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            {filtered.map(t => (
              <TournamentCard key={t.id} tournament={t} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
