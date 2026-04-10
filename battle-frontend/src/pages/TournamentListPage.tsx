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
import { useAuthStore } from '../stores/useAuthStore';

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
        registration_deadline: form.registration_deadline || undefined,
      };
      const t = await createTournament(payload);
      onCreated(t);
    } catch (err: unknown) {
      const msg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message ?? 'Failed to create tournament';
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
          <h2 className="text-xl font-bold text-white">Create Tournament</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white transition-colors text-2xl leading-none">&times;</button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div>
            <label className="block text-sm text-gray-400 mb-1">Name</label>
            <input
              id="create-tournament-name"
              className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB] transition-colors"
              required minLength={3}
              value={form.name}
              onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
              placeholder="KaspaBattle Weekly #1"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-sm text-gray-400 mb-1">Max Teams</label>
              <select
                id="create-tournament-max-teams"
                className="w-full bg-[#1a2332] border border-[#2a3a4a] rounded-lg px-3 py-2 text-white focus:outline-none focus:border-[#49EACB]"
                value={form.max_teams}
                onChange={e => setForm(f => ({ ...f, max_teams: Number(e.target.value) }))}
              >
                {[4, 8, 16].map(n => <option key={n} value={n}>{n} Teams</option>)}
              </select>
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">Buy-In (KAS)</label>
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
            <label className="block text-sm text-gray-400 mb-2">Prize Split</label>
            <div className="grid grid-cols-3 gap-2">
              {(['prize_winner_pct', 'prize_runner_up_pct', 'platform_fee_pct'] as const).map((key, i) => (
                <div key={key}>
                  <label className="block text-xs text-gray-500 mb-1">
                    {['Winner %', 'Runner-up %', 'Fee %'][i]}
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
              <p className="text-orange-400 text-xs mt-1">⚠️ Percentages must sum to 100 (currently {pct})</p>
            )}
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">Registration Deadline (optional)</label>
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
            className="w-full py-3 bg-[#49EACB] hover:bg-[#3dd4b8] text-[#0a0f14] font-bold rounded-lg transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {loading ? 'Creating…' : 'Create Tournament'}
          </button>
        </form>
      </div>
    </div>
  );
}

// ─── Tournament Card ──────────────────────────────────────────────────────────

function TournamentCard({ tournament }: { tournament: Tournament }) {
  const kas = sompiToKas(tournament.buy_in_sompi);
  const pool = sompiToKas(tournament.total_prize_pool_sompi);

  return (
    <Link
      to={`/tournaments/${tournament.id}`}
      id={`tournament-card-${tournament.id}`}
      className="group block bg-[#0d1b2a] border border-white/5 hover:border-[#49EACB]/30 rounded-2xl p-5 transition-all duration-300 hover:shadow-[0_0_24px_rgba(73,234,203,0.08)] hover:-translate-y-0.5"
    >
      <div className="flex items-start justify-between mb-3">
        <div className="flex-1 min-w-0">
          <h3 className="font-bold text-white text-lg truncate group-hover:text-[#49EACB] transition-colors">
            {tournament.name}
          </h3>
          <p className="text-xs text-gray-500 mt-0.5 uppercase tracking-wider">{tournament.game_id}</p>
        </div>
        <StatusBadge status={tournament.status} />
      </div>

      <div className="grid grid-cols-3 gap-3 mt-4">
        <div className="bg-[#0a0f14] rounded-lg p-3 text-center">
          <div className="text-[#49EACB] font-bold text-lg">{kas}</div>
          <div className="text-gray-500 text-xs mt-0.5">Buy-in (KAS)</div>
        </div>
        <div className="bg-[#0a0f14] rounded-lg p-3 text-center">
          <div className="text-white font-bold text-lg">{tournament.max_teams}</div>
          <div className="text-gray-500 text-xs mt-0.5">Max Teams</div>
        </div>
        <div className="bg-[#0a0f14] rounded-lg p-3 text-center">
          <div className="text-[#49EACB] font-bold text-lg">{pool}</div>
          <div className="text-gray-500 text-xs mt-0.5">Prize Pool</div>
        </div>
      </div>

      {tournament.registration_deadline && (
        <div className="mt-3 text-xs text-gray-500 flex items-center gap-1.5">
          <span>⏰</span>
          <span>Deadline: {new Date(tournament.registration_deadline).toLocaleString()}</span>
        </div>
      )}
    </Link>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export function TournamentListPage() {
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

  const filtered = filter === 'ALL' ? tournaments : tournaments.filter(t => t.status === filter);
  const filterOptions: Array<{ value: TournamentStatus | 'ALL'; label: string }> = [
    { value: 'ALL', label: 'All' },
    { value: 'REGISTRATION', label: 'Open' },
    { value: 'IN_PROGRESS', label: 'Live' },
    { value: 'COMPLETED', label: 'Completed' },
  ];

  return (
    <div className="min-h-screen bg-[#070d14] py-10 px-4">
      <div className="max-w-5xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-3xl font-black text-white tracking-tight">
              <span className="text-[#49EACB]">Kaspa</span> Tournaments
            </h1>
            <p className="text-gray-400 text-sm mt-1">Compete, win, earn KAS</p>
          </div>
          <div className="flex items-center gap-3">
            <button
              id="refresh-tournaments-btn"
              onClick={load}
              className="p-2 rounded-lg border border-white/10 hover:border-[#49EACB]/30 text-gray-400 hover:text-[#49EACB] transition-all"
              title="Refresh"
            >
              ↻
            </button>
            {user && (
              <button
                id="create-tournament-btn"
                onClick={() => setShowCreate(true)}
                className="px-5 py-2.5 bg-[#49EACB] hover:bg-[#3dd4b8] text-[#070d14] font-bold rounded-xl transition-all duration-200 text-sm shadow-[0_0_16px_rgba(73,234,203,0.3)]"
              >
                + Create Tournament
              </button>
            )}
          </div>
        </div>

        {/* Filter */}
        <div className="flex gap-2 mb-6">
          {filterOptions.map(opt => (
            <button
              key={opt.value}
              id={`filter-${opt.value.toLowerCase()}`}
              onClick={() => setFilter(opt.value)}
              className={`px-4 py-1.5 rounded-full text-sm font-semibold transition-all duration-200 ${
                filter === opt.value
                  ? 'bg-[#49EACB] text-[#070d14]'
                  : 'bg-white/5 text-gray-400 hover:text-white hover:bg-white/10'
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
              <div key={i} className="bg-[#0d1b2a] rounded-2xl p-5 animate-pulse h-44" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <div className="text-center py-24 text-gray-500">
            <div className="text-5xl mb-4">🏆</div>
            <p className="text-lg font-semibold text-gray-400">No tournaments found</p>
            <p className="text-sm mt-1">
              {user ? 'Be the first to create one!' : 'Sign in to create a tournament'}
            </p>
          </div>
        ) : (
          <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {filtered.map(t => (
              <TournamentCard key={t.id} tournament={t} />
            ))}
          </div>
        )}
      </div>

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
