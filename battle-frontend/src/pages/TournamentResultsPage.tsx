import { useState, useEffect, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import {
  getTournamentResults,
  sompiToKas,
  type TournamentResults,
  type BracketSlot,
} from '../api/tournaments';
import { useTranslation } from 'react-i18next';

// ─── Podium ───────────────────────────────────────────────────────────────────

function Podium({ results }: { results: TournamentResults }) {
  const { t } = useTranslation();
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
          <svg width="36" height="36" viewBox="0 0 36 36" fill="none" xmlns="http://www.w3.org/2000/svg" className="text-[#9ca3af]">
            <circle cx="18" cy="22" r="11" stroke="currentColor" strokeWidth="1.5" fill="none"/>
            <circle cx="18" cy="22" r="7" stroke="currentColor" strokeWidth="1" fill="currentColor" fillOpacity="0.08"/>
            <path d="M13 6h10v4l-5 3-5-3V6Z" stroke="currentColor" strokeWidth="1.2" fill="none" strokeLinejoin="round"/>
          </svg>
          <div className="bg-gradient-to-b from-[#1a2535] to-[#0d1827] border border-[#9ca3af]/30 rounded-2xl p-5 w-36 text-center shadow-xl">
            <div className="text-2xl font-black text-[#9ca3af] mb-1">2nd</div>
            <div className="font-bold text-white text-sm truncate">{runnerUp?.name ?? t('tournaments.detail.bracket.tbd')}</div>
            <div className="text-xs text-gray-500 mt-0.5">{runnerUp?.captain_display_name}</div>
            <div className="text-[#9ca3af] font-bold text-sm mt-3">
              {sompiToKas(results.runner_up_share_sompi)} KAS
            </div>
          </div>
          <div className="h-16 w-36 bg-gradient-to-t from-[#9ca3af]/10 to-transparent rounded-b-xl border border-t-0 border-[#9ca3af]/10" />
        </div>

        {/* Champion */}
        <div className="flex flex-col items-center gap-3 -mt-8">
          <div className="w-16 h-16 rounded-full bg-gradient-to-br from-[#fbbf24] to-[#f59e0b] flex items-center justify-center shadow-[0_0_32px_rgba(251,191,36,0.4)]">
            <svg width="30" height="30" viewBox="0 0 30 30" fill="white" xmlns="http://www.w3.org/2000/svg">
              <path d="M10 4H20V14C20 17.314 17.761 20 15 20C12.239 20 10 17.314 10 14V4Z"/>
              <path d="M10 6H6C6 6 4 9 6 13C6.9 14.6 8.5 15.5 10 15.5" stroke="white" strokeWidth="1.4" fill="none" strokeLinecap="round"/>
              <path d="M20 6H24C24 6 26 9 24 13C23.1 14.6 21.5 15.5 20 15.5" stroke="white" strokeWidth="1.4" fill="none" strokeLinecap="round"/>
              <rect x="13" y="20" width="4" height="3" fill="white"/>
              <rect x="10" y="23" width="10" height="2.5" rx="1.25" fill="white"/>
            </svg>
          </div>
          <div className="bg-gradient-to-b from-[#1f2d1a] to-[#0d1a0f] border border-[#49EACB]/40 rounded-2xl p-6 w-44 text-center shadow-[0_0_48px_rgba(73,234,203,0.12)]">
            <div className="text-3xl font-black text-[#49EACB] mb-1">1st</div>
            <div className="font-bold text-white text-base truncate">{winner?.name ?? t('tournaments.detail.bracket.tbd')}</div>
            <div className="text-xs text-gray-400 mt-0.5">{winner?.captain_display_name}</div>
            <div className="text-[#49EACB] font-black text-lg mt-3">
              {sompiToKas(results.winner_share_sompi)} KAS
            </div>
          </div>
          <div className="h-24 w-44 bg-gradient-to-t from-[#49EACB]/10 to-transparent rounded-b-xl border border-t-0 border-[#49EACB]/10" />
        </div>

        {/* Platform fee visual */}
        <div className="flex flex-col items-center gap-3">
          <svg width="36" height="36" viewBox="0 0 36 36" fill="none" xmlns="http://www.w3.org/2000/svg" className="text-gray-600 opacity-60">
            <rect x="2" y="32" width="32" height="2.5" rx="1.25" fill="currentColor"/>
            <rect x="2" y="13" width="32" height="2.5" rx="1.25" fill="currentColor"/>
            <rect x="8" y="16" width="3" height="16" fill="currentColor"/>
            <rect x="14.5" y="16" width="3" height="16" fill="currentColor"/>
            <rect x="21" y="16" width="3" height="16" fill="currentColor"/>
            <rect x="27" y="16" width="3" height="16" fill="currentColor"/>
            <polygon points="18,4 2,13 34,13" fill="currentColor"/>
          </svg>
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
        {t('tournaments.results.prize_pool')}: <span className="text-[#49EACB] font-bold">{sompiToKas(total)} KAS</span>
      </div>
    </div>
  );
}

// ─── Results Bracket (read-only) ──────────────────────────────────────────────

function ResultsBracket({ slots }: { slots: BracketSlot[] }) {
  const { t } = useTranslation();
  const rounds = Array.from(new Set(slots.map(s => s.round))).sort((a, b) => a - b);
  const maxRound = Math.max(...rounds, 0);

  const roundLabel = (r: number) => {
    const n = slots.filter(s => s.round === r).length;
    if (r === maxRound && n === 1) return t('tournaments.detail.bracket.grand_final');
    if (r === maxRound - 1 && n <= 2) return t('tournaments.detail.bracket.semifinals');
    if (r === maxRound - 2) return t('tournaments.detail.bracket.quarterfinals');
    return t('tournaments.detail.bracket.round', { num: r + 1 });
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
                          {team?.name ?? t('tournaments.detail.bracket.tbd')}
                        </span>
                        {slot.winner_team_id === team?.id && (
                          <svg xmlns="http://www.w3.org/2000/svg" className="w-3.5 h-3.5 text-[#49EACB]" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2.5}><path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" /></svg>
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
  const { t } = useTranslation();
  if (!results.payout_tx_hash) {
    return (
      <div className="bg-[#0d1b2a] border border-orange-500/20 rounded-xl p-5">
        <div className="flex items-center gap-2 text-orange-400 mb-2">
          <svg xmlns="http://www.w3.org/2000/svg" className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.75}><circle cx="12" cy="12" r="9" strokeLinecap="round" strokeLinejoin="round" /><path strokeLinecap="round" strokeLinejoin="round" d="M12 7v5l3 3" /></svg>
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
        <svg xmlns="http://www.w3.org/2000/svg" className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.75}><path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
        <span className="font-bold text-sm">Payout Completed</span>
        {results.payout_executed_at && (
          <span className="text-gray-500 text-xs ml-auto">
            {new Date(results.payout_executed_at).toLocaleString()}
          </span>
        )}
      </div>
      <div>
        <p className="text-xs text-gray-500 mb-1">{t('tournaments.results.payout_tx')}</p>
        <code className="text-xs text-[#49EACB] break-all bg-[#0a0f14] rounded-lg p-3 block font-mono">
          {results.payout_tx_hash}
        </code>
      </div>
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export function TournamentResultsPage() {
  const { t } = useTranslation();
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
          <svg width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg" className="mx-auto mb-4 text-gray-700">
            <circle cx="22" cy="22" r="14" stroke="currentColor" strokeWidth="1.5" fill="none"/>
            <path d="M32 32L42 42" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
          </svg>
          <h2 className="text-xl font-bold text-white mb-2">{t('tournaments.detail.not_found')}</h2>
          <Link to="/tournaments" className="text-[#49EACB] hover:underline text-sm">
            {t('tournaments.results.navbar_back')}
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
            {t('tournaments.results.navbar_back')}
          </Link>
          <span className="text-gray-700">/</span>
          <span className="text-gray-400 text-sm">{t('tournaments.detail.actions.results')}</span>
        </div>

        {/* Title */}
        <div className="text-center">
          <h1 className="text-4xl font-black text-white">
            {t('tournaments.results.title').split(' ').map((word, i) => i === 0 ? <span key={i} className="text-[#49EACB]">{word} </span> : word + ' ')}
          </h1>
          <p className="text-gray-400 mt-2 text-sm">{t('tournaments.results.subtitle')}</p>
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
            {t('tournaments.results.back_overview')}
          </Link>
        </div>
      </div>
    </div>
  );
}
