import { useState, useEffect, useCallback } from 'react';
import { Link } from 'react-router-dom';
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

// ─── Create Tournament Modal ───────────────────────────────────────────────────

interface CreateModalProps {
  onClose: () => void;
  onCreated: (t: Tournament) => void;
}

function CreateTournamentModal({ onClose, onCreated }: CreateModalProps) {
  const { t } = useTranslation();
  const [form, setForm] = useState({
    name: '',
    game_id: 'cs2',
    max_teams: 8,
    buy_in_sompi: 500_000_000, // 5 KAS
    prize_winner_pct: 70,
    prize_runner_up_pct: 20,
    platform_fee_pct: 10,
    registration_deadline: '',
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const payload = {
        ...form,
        registration_deadline: form.registration_deadline 
          ? new Date(form.registration_deadline).toISOString() 
          : undefined,
      };
      const t = await createTournament(payload);
      onCreated(t);
    } catch (err: unknown) {
      const msg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message ?? t('tournaments.create.error_fallback');
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  const pct = form.prize_winner_pct + form.prize_runner_up_pct + form.platform_fee_pct;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
      <div className="bg-[#0f1923] border border-[#49EACB]/20 rounded-2xl w-full max-w-lg shadow-2xl">
        <div className="flex items-center justify-between p-6 border-b border-white/10">
          <div>
            <h2 className="text-xl font-bold text-white">{t('tournaments.create.title')}</h2>
            <p className="text-xs text-[#49EACB]/70 bg-[#49EACB]/5 border border-[#49EACB]/10 rounded-lg px-3 py-2 mt-2">
              {t('tournaments.create.game_hint')}
            </p>
          </div>
          <button onClick={onClose} className="text-gray-400 hover:text-white transition-colors text-2xl leading-none">&times;</button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div>
            <label className="block text-sm text-gray-400 mb-1">{t('tournaments.create.name')}</label>
            <input
              id="create-tournament-name"
              className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB] transition-colors"
              required minLength={3}
              value={form.name}
              onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
              placeholder={t('tournaments.create.name_placeholder')}
            />
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">{t('tournaments.create.game_label')}</label>
            <div className="w-full bg-[#0f1720] border border-[#2a3a4a] rounded-lg px-3 py-2 text-gray-500 text-sm flex items-center justify-between cursor-not-allowed">
              <span>{t('tournaments.create.game_value')}</span>
              <span className="text-[10px] uppercase tracking-widest text-gray-600 bg-white/5 px-1.5 py-0.5 rounded">Only option</span>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-sm text-gray-400 mb-1">{t('tournaments.create.max_teams')}</label>
              <select
                id="create-tournament-max-teams"
                className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB]"
                value={form.max_teams}
                onChange={e => setForm(f => ({ ...f, max_teams: Number(e.target.value) }))}
              >
                {[4, 8, 16].map(n => <option key={n} value={n}>{t('tournaments.create.teams_count', { count: n })}</option>)}
              </select>
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">{t('tournaments.create.buy_in')}</label>
              <input
                id="create-tournament-buyin"
                type="number" min={0} step={0.01}
                className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB]"
                value={form.buy_in_sompi / 1e8}
                onChange={e => setForm(f => ({ ...f, buy_in_sompi: Math.round(Number(e.target.value) * 1e8) }))}
              />
            </div>
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-2">{t('tournaments.create.prize_split')}</label>
            <div className="grid grid-cols-3 gap-2">
              {(['prize_winner_pct', 'prize_runner_up_pct', 'platform_fee_pct'] as const).map((key, i) => (
                <div key={key}>
                  <label className="block text-xs text-gray-500 mb-1">
                    {[t('tournaments.create.winner_pct'), t('tournaments.create.runner_up_pct'), t('tournaments.create.fee_pct')][i]}
                  </label>
                  <input
                    type="number" min={0} max={100}
                    className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB]"
                    value={form[key]}
                    onChange={e => setForm(f => ({ ...f, [key]: Number(e.target.value) }))}
                  />
                </div>
              ))}
            </div>
            {pct !== 100 && (
              <p className="text-orange-400 text-xs mt-1">{t('tournaments.create.pct_warning', { pct })}</p>
            )}
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">{t('tournaments.create.deadline')}</label>
            <input
              type="datetime-local"
              className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB]"
              value={form.registration_deadline}
              onChange={e => setForm(f => ({ ...f, registration_deadline: e.target.value }))}
            />
          </div>
          {error && <p className="text-red-400 text-sm bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2">{error}</p>}
          <button
            id="create-tournament-submit"
            type="submit"
            disabled={loading || pct !== 100 || !form.name}
            className="w-full py-3 bg-[#49EACB] hover:bg-[#3dd4b8] text-[#0a0f14] font-black rounded-lg transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed uppercase tracking-wide"
          >
            {loading ? t('tournaments.create.creating') : t('tournaments.create.submit')}
          </button>
        </form>
      </div>
    </div>
  );
}

// ─── Tournament Card ──────────────────────────────────────────────────────────

function TournamentCard({ tournament }: { tournament: Tournament }) {
  const { t } = useTranslation();
  const kas = sompiToKas(tournament.buy_in_sompi);
  const pool = sompiToKas(tournament.total_prize_pool_sompi);

  return (
    <Link
      to={`/tournaments/${tournament.id}`}
      id={`tournament-card-${tournament.id}`}
      className="group block bg-slate-900/60 border border-slate-700/50 hover:border-kaspa-primary/30 rounded-2xl p-5 transition-all duration-300 hover:bg-slate-800/80 hover:-translate-y-0.5"
    >
      <div className="flex items-start justify-between mb-3">
        <div className="flex-1 min-w-0">
          <h3 className="font-bold text-white text-lg truncate group-hover:text-[#49EACB] transition-colors">
            {tournament.name}
          </h3>
          <p className="text-xs text-gray-500 mt-0.5 uppercase tracking-wider">{tournament.game_id.toUpperCase()}</p>
        </div>
        <StatusBadge status={tournament.status} />
      </div>

      <div className="grid grid-cols-3 gap-3 mt-4">
        <div className="bg-slate-800/60 border border-slate-700/40 rounded-lg p-3 text-center">
          <div className="text-kaspa-primary font-bold text-lg">{kas}</div>
          <div className="text-gray-500 text-xs mt-0.5">{t('tournaments.card.buy_in')}</div>
        </div>
        <div className="bg-slate-800/60 border border-slate-700/40 rounded-lg p-3 text-center">
          <div className="text-white font-bold text-lg">{tournament.max_teams}</div>
          <div className="text-gray-500 text-xs mt-0.5">{t('tournaments.card.max_teams')}</div>
        </div>
        <div className="bg-slate-800/60 border border-slate-700/40 rounded-lg p-3 text-center">
          <div className="text-kaspa-primary font-bold text-lg">{pool}</div>
          <div className="text-gray-500 text-xs mt-0.5">{t('tournaments.card.prize_pool')}</div>
        </div>
      </div>

      {tournament.registration_deadline && (
        <div className="mt-3 text-xs text-gray-500 flex items-center gap-1.5">
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg" className="shrink-0">
            <circle cx="6" cy="6" r="5" stroke="currentColor" strokeWidth="1.2"/>
            <path d="M6 3.5V6.5L8 7.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
          <span>{t('tournaments.card.deadline')}: {new Date(tournament.registration_deadline).toLocaleString()}</span>
        </div>
      )}
    </Link>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export function TournamentListPage() {
  const { t } = useTranslation();
  const [tournaments, setTournaments] = useState<Tournament[]>([]);
  const [loading, setLoading] = useState(true);
  const [showCreate, setShowCreate] = useState(false);
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
      setShowCreate(true);
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

      {/* Content */}
      {loading ? (
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {[...Array(6)].map((_, i) => (
            <div key={i} className="bg-slate-800/40 border border-slate-700/50 rounded-2xl p-5 animate-pulse h-44" />
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
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map(t => (
            <TournamentCard key={t.id} tournament={t} />
          ))}
        </div>
      )}

      {showCreate && (
        <CreateTournamentModal
          onClose={() => setShowCreate(false)}
          onCreated={t => {
            setTournaments(prev => [t, ...prev]);
            setShowCreate(false);
          }}
        />
      )}
    </div>
  );
}
