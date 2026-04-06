import { useState, useEffect, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import {
  getTournamentResults,
  sompiToKas,
  type TournamentResults,
  type BracketSlot,
} from '../api/tournaments';

// ─── Podium ───────────────────────────────────────────────────────────────────

function Podium({ results }: { results: TournamentResults }) {
  const total = results.total_prize_pool_sompi;
  const winner = results.winner_team;
  const runnerUp = results.runner_up_team;

  return (
    <div className="relative">
      {/* Champion glow backdrop */}
      <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
        <div className="w-64 h-64 rounded-full bg-[#49EACB]/5 blur-3xl" />
      </div>

      <div className="relative flex items-end justify-center gap-4 py-8">
        {/* Runner-up */}
        <div className="flex flex-col items-center gap-3">
          <div className="text-3xl">🥈</div>
          <div className="bg-gradient-to-b from-[#1a2535] to-[#0d1827] border border-[#9ca3af]/30 rounded-2xl p-5 w-36 text-center shadow-xl">
            <div className="text-2xl font-black text-[#9ca3af] mb-1">2nd</div>
            <div className="font-bold text-white text-sm truncate">{runnerUp?.name ?? 'TBD'}</div>
            <div className="text-xs text-gray-500 mt-0.5">{runnerUp?.captain_display_name}</div>
            <div className="text-[#9ca3af] font-bold text-sm mt-3">
              {sompiToKas(results.runner_up_share_sompi)} KAS
            </div>
          </div>
          <div className="h-16 w-36 bg-gradient-to-t from-[#9ca3af]/10 to-transparent rounded-b-xl border border-t-0 border-[#9ca3af]/10" />
        </div>

        {/* Champion */}
        <div className="flex flex-col items-center gap-3 -mt-8">
          <div className="w-16 h-16 rounded-full bg-gradient-to-br from-[#fbbf24] to-[#f59e0b] flex items-center justify-center text-3xl shadow-[0_0_32px_rgba(251,191,36,0.4)]">
            🏆
          </div>
          <div className="bg-gradient-to-b from-[#1f2d1a] to-[#0d1a0f] border border-[#49EACB]/40 rounded-2xl p-6 w-44 text-center shadow-[0_0_48px_rgba(73,234,203,0.12)]">
            <div className="text-3xl font-black text-[#49EACB] mb-1">1st</div>
            <div className="font-bold text-white text-base truncate">{winner?.name ?? 'TBD'}</div>
            <div className="text-xs text-gray-400 mt-0.5">{winner?.captain_display_name}</div>
            <div className="text-[#49EACB] font-black text-lg mt-3">
              {sompiToKas(results.winner_share_sompi)} KAS
            </div>
          </div>
          <div className="h-24 w-44 bg-gradient-to-t from-[#49EACB]/10 to-transparent rounded-b-xl border border-t-0 border-[#49EACB]/10" />
        </div>

        {/* Platform fee visual */}
        <div className="flex flex-col items-center gap-3">
          <div className="text-3xl opacity-50">🏛️</div>
          <div className="bg-[#0d1b2a] border border-white/5 rounded-2xl p-5 w-36 text-center">
            <div className="text-2xl font-black text-gray-600 mb-1">Fee</div>
            <div className="font-bold text-gray-500 text-sm">Platform</div>
            <div className="text-gray-500 font-bold text-sm mt-3">
              {sompiToKas(results.platform_fee_sompi)} KAS
            </div>
          </div>
          <div className="h-16 w-36 bg-gradient-to-t from-white/3 to-transparent rounded-b-xl border border-t-0 border-white/5" />
        </div>
      </div>

      <div className="text-center mt-2 text-sm text-gray-500">
        Total Prize Pool: <span className="text-[#49EACB] font-bold">{sompiToKas(total)} KAS</span>
      </div>
    </div>
  );
}

// ─── Results Bracket (read-only) ──────────────────────────────────────────────

function ResultsBracket({ slots }: { slots: BracketSlot[] }) {
  const rounds = Array.from(new Set(slots.map(s => s.round))).sort((a, b) => a - b);
  const maxRound = Math.max(...rounds, 0);

  const roundLabel = (r: number) => {
    const n = slots.filter(s => s.round === r).length;
    if (r === maxRound && n === 1) return 'Grand Final';
    if (r === maxRound - 1 && n <= 2) return 'Semifinals';
    if (r === maxRound - 2) return 'Quarterfinals';
    return `Round ${r + 1}`;
  };

  return (
    <div className="overflow-x-auto pb-4">
      <div className="flex gap-6 min-w-max">
        {rounds.map(round => (
          <div key={round} className="flex flex-col min-w-[180px]">
            <div className="text-center mb-4">
              <span className="text-xs font-bold text-[#49EACB] uppercase tracking-widest">
                {roundLabel(round)}
              </span>
            </div>
            <div className="flex flex-col gap-4 justify-around flex-1">
              {slots
                .filter(s => s.round === round)
                .sort((a, b) => a.slot_index - b.slot_index)
                .map(slot => (
                  <div
                    key={slot.id}
                    className={`rounded-xl border p-3 ${
                      slot.status === 'COMPLETED'
                        ? 'border-[#49EACB]/20 bg-[#49EACB]/3'
                        : 'border-white/5 bg-[#0d1b2a]'
                    }`}
                  >
                    {[slot.team_a, slot.team_b].map((team, i) => (
                      <div
                        key={i}
                        className={`flex items-center justify-between py-1.5 px-2 rounded-lg ${
                          i === 0 ? 'mb-1' : ''
                        } ${
                          slot.winner_team_id === team?.id
                            ? 'bg-[#49EACB]/10'
                            : 'bg-[#0a0f14]'
                        }`}
                      >
                        <span className={`text-xs font-semibold ${!team ? 'text-gray-600 italic' : 'text-white'}`}>
                          {team?.name ?? 'TBD'}
                        </span>
                        {slot.winner_team_id === team?.id && (
                          <span className="text-[#49EACB] text-xs">✓</span>
                        )}
                      </div>
                    ))}
                    {slot.reported_score && (
                      <div className="text-center text-xs text-gray-600 mt-1">{slot.reported_score}</div>
                    )}
                  </div>
                ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── Payout Section ───────────────────────────────────────────────────────────

function PayoutSection({ results }: { results: TournamentResults }) {
  if (!results.payout_tx_hash) {
    return (
      <div className="bg-[#0d1b2a] border border-orange-500/20 rounded-xl p-5">
        <div className="flex items-center gap-2 text-orange-400 mb-2">
          <span className="text-lg">⏳</span>
          <span className="font-bold text-sm">Payout Pending</span>
        </div>
        <p className="text-sm text-gray-400">
          The prize payout transaction is being prepared. This typically takes a few minutes after tournament completion.
        </p>
      </div>
    );
  }

  return (
    <div className="bg-[#0d1b2a] border border-[#49EACB]/20 rounded-xl p-5">
      <div className="flex items-center gap-2 text-[#49EACB] mb-3">
        <span className="text-lg">✅</span>
        <span className="font-bold text-sm">Payout Completed</span>
        {results.payout_executed_at && (
          <span className="text-gray-500 text-xs ml-auto">
            {new Date(results.payout_executed_at).toLocaleString()}
          </span>
        )}
      </div>
      <div>
        <p className="text-xs text-gray-500 mb-1">Transaction ID</p>
        <code className="text-xs text-[#49EACB] break-all bg-[#0a0f14] rounded-lg p-3 block font-mono">
          {results.payout_tx_hash}
        </code>
      </div>
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export function TournamentResultsPage() {
  const { id } = useParams<{ id: string }>();
  const [results, setResults] = useState<TournamentResults | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!id) return;
    try {
      const data = await getTournamentResults(id);
      setResults(data);
    } catch { /* ignore */ } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => { load(); }, [load]);

  if (loading) {
    return (
      <div className="min-h-screen bg-[#070d14] flex items-center justify-center">
        <div className="w-48 h-1 bg-[#49EACB]/20 rounded-full overflow-hidden">
          <div className="h-full bg-[#49EACB] animate-[shimmer_2s_infinite] w-full origin-left" />
        </div>
      </div>
    );
  }

  if (!results) {
    return (
      <div className="min-h-screen bg-[#070d14] flex items-center justify-center">
        <div className="text-center">
          <div className="text-5xl mb-4">🔍</div>
          <h2 className="text-xl font-bold text-white mb-2">Results not found</h2>
          <Link to="/tournaments" className="text-[#49EACB] hover:underline text-sm">
            ← Back to Tournaments
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-[#070d14] py-8 px-4">
      <div className="max-w-4xl mx-auto space-y-8">

        {/* Breadcrumb */}
        <div className="flex items-center gap-3">
          <Link to={`/tournaments/${id}`} className="text-gray-500 hover:text-[#49EACB] text-sm transition-colors">
            ← Tournament
          </Link>
          <span className="text-gray-700">/</span>
          <span className="text-gray-400 text-sm">Results</span>
        </div>

        {/* Title */}
        <div className="text-center">
          <h1 className="text-4xl font-black text-white">
            Tournament <span className="text-[#49EACB]">Results</span>
          </h1>
          <p className="text-gray-400 mt-2 text-sm">Final standings and prize distribution</p>
        </div>

        {/* Podium */}
        {(results.winner_team || results.runner_up_team) && (
          <div className="bg-[#0d1b2a] border border-white/5 rounded-2xl p-6">
            <Podium results={results} />
          </div>
        )}

        {/* Payout Status */}
        <PayoutSection results={results} />

        {/* Bracket recap */}
        {results.bracket.length > 0 && (
          <div className="bg-[#0d1b2a] border border-white/5 rounded-2xl p-6">
            <h2 className="text-lg font-bold text-white mb-5">
              Match Results <span className="text-gray-500 text-sm font-normal ml-2">— Full Bracket</span>
            </h2>
            <ResultsBracket slots={results.bracket} />
          </div>
        )}

        {/* Back button */}
        <div className="text-center">
          <Link
            to="/tournaments"
            className="inline-block px-6 py-3 bg-[#49EACB] hover:bg-[#3dd4b8] text-[#070d14] font-bold rounded-xl transition-all shadow-[0_0_16px_rgba(73,234,203,0.3)]"
          >
            Browse Tournaments
          </Link>
        </div>
      </div>
    </div>
  );
}
