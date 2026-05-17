import { useState, useEffect, useCallback, useRef } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '../hooks/useWallet';
import {
  getTournament,
  listTeams,
  getBracket,
  registerTeam,
  lockBracket,
  submitBracketMatchId,
  fileBracketDispute,
  fileTournamentDispute,
  cancelTournament,
  sompiToKas,
  STATUS_LABELS,
  STATUS_COLORS,
  type Tournament,
  type TournamentTeam,
  type BracketSlot,
  type TournamentStatus,
} from '../api/tournaments';
import { SUPPORTED_GAMES } from '../config/constants';

import { useAuthStore } from '../stores/useAuthStore';
import { useTranslation } from 'react-i18next';

const API_BASE = import.meta.env.VITE_API_BASE_URL || '/api/v1';

// ─── Status badge ─────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: TournamentStatus }) {
  const color = STATUS_COLORS[status] ?? '#6b7280';
  return (
    <span
      style={{ color, borderColor: color + '44', backgroundColor: color + '18' }}
      className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-sm font-semibold border"
    >
      <span style={{ backgroundColor: color }} className="w-2 h-2 rounded-full animate-pulse" />
      {STATUS_LABELS[status] ?? status}
    </span>
  );
}

// ─── Winner Star Badge ────────────────────────────────────────────────────────

function WinnerBadge() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg" className="text-kaspa-primary shrink-0">
      <path
        d="M7 1.5L8.545 4.656L12.04 5.163L9.52 7.618L10.09 11.099L7 9.475L3.91 11.099L4.48 7.618L1.96 5.163L5.455 4.656L7 1.5Z"
        fill="currentColor"
        stroke="currentColor"
        strokeWidth="0.3"
        strokeLinejoin="round"
      />
    </svg>
  );
}

interface BracketCardProps {
  slot: BracketSlot;
  tournamentId: string;
  isCaptain: boolean;
  myTeamIds: string[];
  onRefresh: () => void;
}

function BracketCard({ slot, tournamentId, isCaptain, myTeamIds, onRefresh }: BracketCardProps) {
  const { t } = useTranslation();
  const [showMatchInput, setShowMatchInput] = useState(false);
  const [matchId, setMatchId] = useState('');
  const [showDisputeInput, setShowDisputeInput] = useState(false);
  const [disputeReason, setDisputeReason] = useState('');
  const [loading, setLoading] = useState(false);

  const isMySlot = myTeamIds.some(id => id === slot.team_a?.id || id === slot.team_b?.id);
  const isDone = slot.status === 'COMPLETED';
  const isReady = slot.status === 'READY' || slot.status === 'IN_PROGRESS';

  const handleSubmitMatchId = async () => {
    if (!matchId.trim()) return;
    setLoading(true);
    try {
      await submitBracketMatchId(tournamentId, slot.id, matchId.trim());
      setShowMatchInput(false);
      setMatchId('');
      onRefresh();
    } catch { /* ignore */ } finally { setLoading(false); }
  };

  const handleDispute = async () => {
    if (disputeReason.trim().length < 10) return;
    setLoading(true);
    try {
      await fileBracketDispute(tournamentId, slot.id, disputeReason.trim());
      setShowDisputeInput(false);
      setDisputeReason('');
      onRefresh();
    } catch { /* ignore */ } finally { setLoading(false); }
  };

  const teamA = slot.team_a;
  const teamB = slot.team_b;
  const winner = slot.winner_team_id;

  return (
    <div
      id={`bracket-slot-${slot.id}`}
      className={`relative glass-panel rounded-2xl border transition-all duration-300 shadow-glow-primary ${
        slot.disputed
          ? 'border-orange-500/50 bg-orange-950/20'
          : isDone
          ? 'border-kaspa-primary/30 bg-kaspa-primary/5'
          : isReady
          ? 'border-blue-500/30 bg-blue-950/10'
          : 'border-white/5 bg-slate-900/60'
      }`}
    >
      {/* Round label */}
      <div className="absolute -top-3 left-3">
        <span className="text-[10px] uppercase tracking-widest text-gray-500 bg-kaspa-dark px-2 font-bold">
          {slot.status === 'WAITING' ? t('tournaments.detail.bracket.waiting') : slot.faceit_match_id ? t('tournaments.detail.bracket.match', { id: slot.faceit_match_id.slice(0, 8) }) : t('tournaments.detail.bracket.slot', { num: slot.slot_index + 1 })}
        </span>
      </div>

      <div className="p-4 pt-5">
        {/* Team A */}
        <div className={`flex items-center justify-between py-2 px-3 rounded-lg mb-1 ${
          winner === teamA?.id ? 'bg-kaspa-primary/10 border border-kaspa-primary/30' : 'bg-kaspa-dark/80'
        }`}>
          <span className={`font-semibold text-sm ${!teamA ? 'text-gray-600 italic' : 'text-white'}`}>
            {teamA?.name ?? t('tournaments.detail.bracket.tbd')}
            {teamA?.seed != null && <span className="ml-1.5 text-gray-500 text-xs">#{teamA.seed}</span>}
          </span>
          {winner === teamA?.id && <WinnerBadge />}
        </div>

        {/* VS divider */}
        <div className="text-center text-xs text-gray-600 font-bold my-1">
          {slot.reported_score ? (
            <span className="text-gray-400">{slot.reported_score}</span>
          ) : 'VS'}
        </div>

        {/* Team B */}
        <div className={`flex items-center justify-between py-2 px-3 rounded-lg ${
          winner === teamB?.id ? 'bg-kaspa-primary/10 border border-kaspa-primary/30' : 'bg-kaspa-dark/80'
        }`}>
          <span className={`font-semibold text-sm ${!teamB ? 'text-gray-600 italic' : 'text-white'}`}>
            {teamB?.name ?? t('tournaments.detail.bracket.tbd')}
            {teamB?.seed != null && <span className="ml-1.5 text-gray-500 text-xs">#{teamB.seed}</span>}
          </span>
          {winner === teamB?.id && <WinnerBadge />}
        </div>

        {/* Captain actions */}
        {isCaptain && isMySlot && isReady && !isDone && !slot.disputed && (
          <div className="mt-3 space-y-2">
            {!showMatchInput && !showDisputeInput && (
              <div className="flex gap-2">
                <button
                  id={`submit-match-id-${slot.id}`}
                  onClick={() => setShowMatchInput(true)}
                  className="flex-1 text-xs py-1.5 px-3 bg-blue-600/20 hover:bg-blue-600/30 border border-blue-500/30 text-blue-400 rounded-lg transition-all"
                >
                  {t('tournaments.detail.bracket.submit_id')}
                </button>
                <button
                  id={`dispute-slot-${slot.id}`}
                  onClick={() => setShowDisputeInput(true)}
                  className="text-xs py-1.5 px-3 bg-orange-600/20 hover:bg-orange-600/30 border border-orange-500/30 text-orange-400 rounded-lg transition-all"
                >
                  {t('tournaments.detail.bracket.dispute')}
                </button>
              </div>
            )}

            {showMatchInput && (
              <div className="space-y-2">
                <input
                  id={`match-id-input-${slot.id}`}
                  className="w-full bg-kaspa-dark border border-blue-500/30 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none focus:border-blue-400"
                  placeholder={t('tournaments.detail.bracket.placeholder_id')}
                  value={matchId}
                  onChange={e => setMatchId(e.target.value)}
                />
                <div className="flex gap-2">
                  <button onClick={handleSubmitMatchId} disabled={loading} className="flex-1 text-xs py-1.5 bg-blue-600 hover:bg-blue-500 text-white rounded-lg disabled:opacity-50 transition-all">
                    {loading ? '…' : t('tournaments.detail.bracket.submit_btn')}
                  </button>
                  <button onClick={() => setShowMatchInput(false)} className="text-xs py-1.5 px-3 bg-white/5 hover:bg-white/10 text-gray-400 rounded-lg">
                    {t('tournaments.detail.bracket.cancel')}
                  </button>
                </div>
              </div>
            )}

            {showDisputeInput && (
              <div className="space-y-2">
                <textarea
                  id={`dispute-reason-${slot.id}`}
                  className="w-full bg-kaspa-dark border border-orange-500/30 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none focus:border-orange-400 resize-none"
                  rows={2}
                  placeholder={t('tournaments.detail.bracket.placeholder_dispute')}
                  value={disputeReason}
                  onChange={e => setDisputeReason(e.target.value)}
                />
                <div className="flex gap-2">
                  <button onClick={handleDispute} disabled={loading || disputeReason.trim().length < 10} className="flex-1 text-xs py-1.5 bg-orange-600 hover:bg-orange-500 text-white rounded-lg disabled:opacity-50 transition-all">
                    {loading ? '…' : t('tournaments.detail.bracket.file_dispute')}
                  </button>
                  <button onClick={() => setShowDisputeInput(false)} className="text-xs py-1.5 px-3 bg-white/5 hover:bg-white/10 text-gray-400 rounded-lg">
                    {t('tournaments.detail.bracket.cancel')}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {slot.disputed && (
          <div className="mt-2 text-xs text-orange-400 bg-orange-950/30 border border-orange-500/20 rounded-lg px-3 py-2 flex items-center gap-1.5">
            {t('tournaments.detail.bracket.under_dispute')}
          </div>
        )}

        {isDone && slot.match_finished_at && (
          <div className="mt-2 text-xs text-gray-600 flex items-center gap-1">
            {t('tournaments.detail.bracket.finished')} {new Date(slot.match_finished_at).toLocaleTimeString()}
          </div>
        )}
      </div>
    </div>
  );
}

// ─── Single-Elimination Bracket ───────────────────────────────────────────────

interface BracketViewProps {
  slots: BracketSlot[];
  tournamentId: string;
  isCaptain: boolean;
  myTeamIds: string[];
  onRefresh: () => void;
}

function BracketView({ slots, tournamentId, isCaptain, myTeamIds, onRefresh }: BracketViewProps) {
  const { t } = useTranslation();
  const rounds = Array.from(new Set(slots.map(s => s.round))).sort((a, b) => a - b);

  if (rounds.length === 0) {
    return (
      <div className="text-center py-16 text-gray-500">
        <svg width="40" height="40" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg" className="mx-auto mb-3 text-gray-700">
          <rect x="4" y="8" width="32" height="28" rx="3" stroke="currentColor" strokeWidth="1.5" fill="none"/>
          <path d="M4 14H36" stroke="currentColor" strokeWidth="1.5"/>
          <rect x="12" y="4" width="2" height="8" rx="1" fill="currentColor"/>
          <rect x="26" y="4" width="2" height="8" rx="1" fill="currentColor"/>
          <rect x="9" y="19" width="8" height="2" rx="1" fill="currentColor" opacity="0.6"/>
          <rect x="23" y="19" width="8" height="2" rx="1" fill="currentColor" opacity="0.6"/>
        </svg>
        <p>{t('tournaments.detail.bracket.empty_title')}</p>
      </div>
    );
  }

  const roundLabels: Record<number, string> = {};
  const maxRound = Math.max(...rounds);
  rounds.forEach(r => {
    const slotsInRound = slots.filter(s => s.round === r).length;
    if (r === maxRound && slotsInRound === 1) roundLabels[r] = t('tournaments.detail.bracket.grand_final');
    else if (r === maxRound - 1 && slotsInRound <= 2) roundLabels[r] = t('tournaments.detail.bracket.semifinals');
    else if (r === maxRound - 2) roundLabels[r] = t('tournaments.detail.bracket.quarterfinals');
    else roundLabels[r] = t('tournaments.detail.bracket.round', { num: r + 1 });
  });

  return (
    <div className="overflow-x-auto pb-4">
      <div className="flex gap-6 min-w-max">
        {rounds.map(round => (
          <div key={round} className="flex flex-col">
            <div className="text-center mb-4">
              <span className="text-sm font-bold text-kaspa-primary uppercase tracking-wider">
                {roundLabels[round]}
              </span>
            </div>
            <div className="flex flex-col gap-4 justify-around flex-1">
              {slots
                .filter(s => s.round === round)
                .sort((a, b) => a.slot_index - b.slot_index)
                .map(slot => (
                  <BracketCard
                    key={slot.id}
                    slot={slot}
                    tournamentId={tournamentId}
                    isCaptain={isCaptain}
                    myTeamIds={myTeamIds}
                    onRefresh={onRefresh}
                  />
                ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── Teams List ───────────────────────────────────────────────────────────────

function TeamsList({ teams, myTeamIds }: { teams: TournamentTeam[]; myTeamIds: string[] }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-3">
      {teams.map(team => (
        <div
          key={team.id}
          className={`flex items-center justify-between glass-panel rounded-2xl p-4 border shadow-glow-primary transition-all hover:bg-white/5 ${
            myTeamIds.includes(team.id)
              ? 'border-kaspa-primary/30'
              : 'border-white/5'
          }`}
        >
          <div>
            <div className="flex items-center gap-2">
              <span className="font-semibold text-white">{team.name}</span>
              {myTeamIds.includes(team.id) && (
                <span className="text-[10px] bg-kaspa-primary/20 text-kaspa-primary px-1.5 py-0.5 rounded font-bold">{t('tournaments.detail.teams.you_badge')}</span>
              )}
              {team.seed != null && (
                <span className="text-xs text-gray-500">{t('tournaments.detail.teams.seed', { seed: team.seed })}</span>
              )}
            </div>
            <div className="text-xs text-gray-500 mt-0.5">{t('tournaments.detail.teams.captain', { name: team.captain_display_name ?? t('tournaments.detail.teams.unknown') })}</div>
          </div>
          <div className={`flex items-center gap-1.5 text-xs font-semibold ${
            team.deposit_status === 'CONFIRMED' ? 'text-kaspa-primary' : 'text-orange-400'
          }`}>
            {team.deposit_status === 'CONFIRMED' ? t('tournaments.detail.teams.funded') : t('tournaments.detail.teams.awaiting_deposit')}
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Prize Pool Banner ────────────────────────────────────────────────────────

function PrizePoolBanner({ tournament }: { tournament: Tournament }) {
  const { t } = useTranslation();
  const isFinalPool = ['COMPLETED', 'BRACKET_READY', 'IN_PROGRESS', 'FUNDED'].includes(tournament.status);
  const total = isFinalPool ? tournament.total_prize_pool_sompi : (tournament.buy_in_sompi * tournament.max_teams);
  const winnerAmt = Math.floor(total * tournament.prize_winner_pct / 100);
  const runnerAmt = Math.floor(total * tournament.prize_runner_up_pct / 100);
  const feeAmt = total - winnerAmt - runnerAmt;

  return (
    <div className="grid grid-cols-3 gap-3">
      {[
        { label: t('tournaments.detail.prize_banner.winner'), amount: winnerAmt, pct: tournament.prize_winner_pct, color: '#fbbf24' },
        { label: t('tournaments.detail.prize_banner.runner_up'), amount: runnerAmt, pct: tournament.prize_runner_up_pct, color: '#9ca3af' },
        { label: t('tournaments.detail.prize_banner.fee'), amount: feeAmt, pct: tournament.platform_fee_pct, color: '#6b7280' },
      ].map(item => (
        <div key={item.label} className="glass-panel border border-kaspa-primary/20 rounded-2xl p-6 text-center shadow-glow-primary transition-all hover:bg-white/5">
          <div className="text-sm text-gray-400 mb-1">{item.label}</div>
          <div style={{ color: item.color }} className="text-xl font-black">
            {sompiToKas(item.amount)} KAS
          </div>
          <div className="text-xs text-gray-600 mt-1">{item.pct}%</div>
        </div>
      ))}
    </div>
  );
}

// ─── Team Deposit Block ─────────────────────────────────────────────────────────

function DepositBlock({ tournament, myTeam, onDepositSuccess }: { tournament: Tournament; myTeam: TournamentTeam; onDepositSuccess: () => void }) {
  const { t } = useTranslation();
  const { signAndSendTournamentDeposit } = useWallet();
  const [isDepositing, setIsDepositing] = useState(false);
  const [depositError, setDepositError] = useState<string | null>(null);

  const buyInKas = sompiToKas(tournament.buy_in_sompi);
  const amountKas = tournament.buy_in_sompi / 100_000_000;

  const handlePayNow = async () => {
    if (!myTeam) return;
    console.log(`[DepositBlock] Paying for tournamentId=${tournament.id} teamId=${myTeam.id}`);

    setDepositError(null);
    setIsDepositing(true);
    try {
      if (!tournament.escrow_address) {
        throw new Error("Turnier hat keine Escrow Adresse.");
      }
      await signAndSendTournamentDeposit(tournament.id, myTeam.id, amountKas, tournament.escrow_address);
      onDepositSuccess();
    } catch (e: unknown) {
      const err = e as { message?: string; response?: { data?: { error?: string; message?: string } } };
      // Try to extract specific error code from backend response
      const apiError = err?.response?.data?.error;
      const apiMessage = err?.response?.data?.message;
      if (apiError === 'team_not_in_tournament') {
        setDepositError(
          t('tournaments.detail.teams.error_team_not_found') ||
          'Dein Team existiert in diesem Turnier nicht mehr. Bitte Seite neu laden oder Team neu registrieren.'
        );
      } else {
        setDepositError(apiMessage || err.message || 'Tournament deposit failed');
      }

    } finally {
      setIsDepositing(false);
    }
  };

  return (
    <div className="mt-6 glass-panel border border-kaspa-primary/20 rounded-2xl p-6 shadow-glow-primary">
      <h3 className="text-lg font-bold text-white mb-4">{t('tournaments.detail.teams.deposit_title')}</h3>
      <div className="flex flex-col md:flex-row gap-6">
        <div className="flex-1 space-y-4">
          <div>
            <p className="text-xs text-gray-500 uppercase tracking-wider mb-1">{t('tournaments.detail.overview.escrow_address')}</p>
            <code className="block text-xs text-kaspa-primary break-all bg-kaspa-dark/80 border border-slate-700/50 rounded-xl p-3">
              {tournament.escrow_address}
            </code>
          </div>
          <div className="flex justify-between items-center bg-kaspa-dark/50 border border-slate-700/50 rounded-xl p-3">
            <span className="text-sm text-gray-400">{t('tournaments.detail.teams.stake_per_team')}</span>
            <span className="font-bold text-emerald-400">{buyInKas} KAS</span>
          </div>
          {depositError && (
            <div className="p-3 bg-red-900/20 border border-red-900/50 rounded-lg">
              <p className="text-red-400 text-xs font-medium">{depositError}</p>
            </div>
          )}
        </div>
        <div className="w-full md:w-64 shrink-0 flex flex-col justify-between">
          <div className="mb-4 md:mb-0 text-center md:text-right">
            <p className="text-xs text-gray-500 uppercase tracking-wider mb-1">Status</p>
            {myTeam.deposit_status === 'CONFIRMED' ? (
              <span className="inline-flex items-center gap-1.5 text-kaspa-primary font-bold">
                <span className="w-2 h-2 rounded-full bg-kaspa-primary animate-pulse" />
                {t('tournaments.detail.teams.funded')}
              </span>
            ) : (
              <span className="inline-flex items-center gap-1.5 text-orange-400 font-bold">
                <span className="w-2 h-2 rounded-full bg-orange-400 animate-pulse" />
                {t('tournaments.detail.teams.awaiting_deposit')}
              </span>
            )}
          </div>
          {myTeam.deposit_status === 'PENDING' && (
            <button
              onClick={handlePayNow}
              disabled={isDepositing}
              className="w-full bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark font-black uppercase tracking-wider py-3 rounded-xl transition-all disabled:opacity-50 disabled:grayscale"
            >
              {isDepositing ? t('deposit.signing') || 'Signing...' : t('tournaments.detail.teams.deposit_now')}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export function TournamentDetailPage() {
  const { id } = useParams<{ id: string }>();
  const user = useAuthStore(s => s.user);
  const { t } = useTranslation();

  const [tournament, setTournament] = useState<Tournament | null>(null);
  const [teams, setTeams] = useState<TournamentTeam[]>([]);
  const [bracket, setBracket] = useState<BracketSlot[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'overview' | 'teams' | 'bracket'>('overview');

  // Team registration
  const [showRegister, setShowRegister] = useState(false);
  const [teamName, setTeamName] = useState('');
  const [regLoading, setRegLoading] = useState(false);
  const [regError, setRegError] = useState('');

  // Tournament dispute
  const [showDispute, setShowDispute] = useState(false);
  const [disputeReason, setDisputeReason] = useState('');

  const wsRef = useRef<WebSocket | null>(null);

  const load = useCallback(async () => {
    if (!id) return;
    try {
      const [t, ts, b] = await Promise.all([getTournament(id), listTeams(id), getBracket(id)]);
      setTournament(t);
      setTeams(ts);
      setBracket(b);
    } catch {
      // Could not load — handled by null check below
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => { load(); }, [load]);

  // WebSocket for real-time bracket updates
  useEffect(() => {
    if (!id) return;
    const wsBase = (API_BASE || '').replace(/^https?/, 'wss').replace(/^http/, 'ws').replace('/api/v1', '');
    const ws = new WebSocket(`${wsBase}/ws`);
    wsRef.current = ws;
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (
          msg.tournament_id === id &&
          ['bracket_result', 'tournament_completed', 'bracket_disputed', 'tournament_disputed'].includes(msg.type)
        ) {
          load();
        }
      } catch { /* ignore */ }
    };
    return () => ws.close();
  }, [id, load]);

  const myTeamIds = teams
    .filter(t => t.captain_user_id === user?.id)
    .map(t => t.id);
  const myTeam = teams.find(t => t.captain_user_id === user?.id);
  const isCaptain = myTeamIds.length > 0;
  const isOrganizer = tournament?.organizer_user_id === user?.id;
  const isLockable =
    tournament?.status === 'REGISTRATION' || tournament?.status === 'FUNDED';
  const isCancellable = isOrganizer && ['REGISTRATION', 'FUNDED', 'DRAFT'].includes(tournament?.status ?? '');

  const handleRegister = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!id || !teamName.trim()) return;
    setRegLoading(true);
    setRegError('');
    try {
      await registerTeam(id, teamName.trim());
      setShowRegister(false);
      setTeamName('');
      await load();
    } catch (err: unknown) {
      const errData = (err as { response?: { data?: { error?: string; message?: string } } })?.response?.data;
      const codeMap: Record<string, string> = {
        registration_closed: t('tournaments.detail.teams.error_registration_closed'),
        tournament_full: t('tournaments.detail.teams.error_tournament_full'),
        team_name_taken: t('tournaments.detail.teams.error_name_taken'),
      };
      const code = errData?.error ?? '';
      setRegError(codeMap[code] ?? errData?.message ?? t('tournaments.detail.teams.error_generic'));
    } finally {
      setRegLoading(false);
    }
  };

  const handleLock = async () => {
    if (!id) return;
    if (!confirm(t('tournaments.detail.confirm.lock'))) return;
    try {
      await lockBracket(id);
      await load();
    } catch { /* ignore */ }
  };

  const handleCancel = async () => {
    if (!id) return;
    if (!confirm(t('tournaments.detail.confirm.cancel'))) return;
    try {
      await cancelTournament(id);
      await load();
    } catch { /* ignore */ }
  };

  const handleDispute = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!id || disputeReason.trim().length < 10) return;
    try {
      await fileTournamentDispute(id, disputeReason.trim());
      setShowDispute(false);
      setDisputeReason('');
      await load();
    } catch { /* ignore */ }
  };

  if (loading) {
    return (
      <div className="min-h-[60vh] flex items-center justify-center">
        <div className="w-48 h-1 bg-kaspa-primary/20 rounded-full overflow-hidden">
          <div className="h-full bg-kaspa-primary animate-[shimmer_2s_infinite] w-full origin-left" />
        </div>
      </div>
    );
  }

  if (!tournament) {
    return (
      <div className="min-h-[60vh] flex items-center justify-center text-center">
        <div>
          <svg width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg" className="mx-auto mb-4 text-gray-700">
            <circle cx="22" cy="22" r="14" stroke="currentColor" strokeWidth="1.5" fill="none"/>
            <path d="M32 32L42 42" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
          </svg>
          <h2 className="text-xl font-bold text-white mb-2">{t('tournaments.detail.not_found')}</h2>
          <Link to="/tournaments" className="text-kaspa-primary hover:underline text-sm">
            {t('tournaments.detail.navbar_back')}
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="max-w-5xl mx-auto space-y-6">

        <Link to="/tournaments" className="text-gray-500 hover:text-kaspa-primary text-sm flex items-center gap-1.5 transition-colors w-fit">
          {t('tournaments.detail.navbar_back')}
        </Link>

        {/* Header */}
        <div className="mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary">
          <div className="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
            <div>
              <div className="flex items-center gap-3 flex-wrap">
                <h1 className="text-2xl font-black text-white">{tournament.name}</h1>
                <StatusBadge status={tournament.status} />
              </div>
              <div className="flex items-center gap-4 mt-2 text-sm text-gray-400">
                <span className="uppercase tracking-wider">{tournament.game_id}</span>
                <span>·</span>
                <span>{teams.length} / {tournament.max_teams} {t('tournaments.detail.teams_count')}</span>
                {tournament.registration_deadline && (
                  <>
                    <span>·</span>
                    <span>{t('tournaments.card.deadline')}: {new Date(tournament.registration_deadline).toLocaleString()}</span>
                  </>
                )}
              </div>
            </div>

            {/* Action buttons */}
            <div className="flex flex-wrap gap-2">
              {tournament.status === 'COMPLETED' && (
                <Link
                  to={`/tournaments/${id}/results`}
                  id="view-results-btn"
                  className="px-4 py-2 bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark font-bold rounded-xl text-sm transition-all"
                >
                  {t('tournaments.detail.actions.results')}
                </Link>
              )}
              {user && tournament.status === 'REGISTRATION' && !isCaptain &&
                teams.length < tournament.max_teams &&
                (!tournament.registration_deadline || new Date() < new Date(tournament.registration_deadline)) && (
                <button
                  id="register-team-btn"
                  onClick={() => { setActiveTab('teams'); setShowRegister(true); }}
                  className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-bold rounded-xl text-sm transition-all"
                >
                  {t('tournaments.detail.actions.register')}
                </button>
              )}
              {isOrganizer && isLockable && teams.length >= 2 && (
                <button
                  id="lock-bracket-btn"
                  onClick={handleLock}
                  className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white font-bold rounded-xl text-sm transition-all"
                >
                  {t('tournaments.detail.actions.lock_bracket')}
                </button>
              )}
              {isCaptain && ['IN_PROGRESS', 'BRACKET_READY', 'COMPLETED'].includes(tournament.status) && !showDispute && (
                <button
                  id="dispute-tournament-btn"
                  onClick={() => setShowDispute(true)}
                  className="px-4 py-2 bg-orange-600/20 hover:bg-orange-600/30 border border-orange-500/30 text-orange-400 font-semibold rounded-xl text-sm transition-all"
                >
                  {t('tournaments.detail.actions.dispute')}
                </button>
              )}
              {isCancellable && (
                <button
                  id="cancel-tournament-btn"
                  onClick={handleCancel}
                  className="px-4 py-2 bg-red-600/20 hover:bg-red-600/30 border border-red-500/30 text-red-400 font-semibold rounded-xl text-sm transition-all"
                >
                  {t('tournaments.detail.actions.cancel')}
                </button>
              )}
            </div>
          </div>

          {/* Dispute form inline */}
          {showDispute && (
            <form onSubmit={handleDispute} className="mt-4 bg-orange-950/20 border border-orange-500/20 rounded-xl p-4 space-y-3">
              <p className="text-sm text-orange-300 font-semibold">{t('tournaments.detail.dispute.title')}</p>
              <textarea
                id="tournament-dispute-reason"
                className="w-full bg-kaspa-dark border border-orange-500/30 rounded-lg px-3 py-2 text-sm text-white focus:outline-none resize-none"
                rows={3}
                placeholder={t('tournaments.detail.dispute.placeholder')}
                value={disputeReason}
                onChange={e => setDisputeReason(e.target.value)}
              />
              <div className="flex gap-2">
                <button
                  type="submit"
                  disabled={disputeReason.trim().length < 10}
                  className="px-4 py-1.5 bg-orange-600 hover:bg-orange-500 text-white font-bold rounded-lg text-sm disabled:opacity-40 transition-all"
                >
                  {t('tournaments.detail.dispute.submit')}
                </button>
                <button
                  type="button"
                  onClick={() => setShowDispute(false)}
                  className="px-4 py-1.5 bg-white/5 hover:bg-white/10 text-gray-400 rounded-lg text-sm"
                >
                  {t('tournaments.detail.dispute.cancel')}
                </button>
              </div>
            </form>
          )}
        </div>

        {/* Prize Pool */}
        <PrizePoolBanner tournament={tournament} />

        {/* Tabs */}
        <div className="flex gap-1 bg-slate-800/40 p-1 rounded-xl border border-slate-700/50 w-fit">
          {(['overview', 'teams', 'bracket'] as const).map(tab => (
            <button
              key={tab}
              id={`tab-${tab}`}
              onClick={() => setActiveTab(tab)}
              className={`px-5 py-2 rounded-lg text-sm font-semibold capitalize transition-all ${
                activeTab === tab
                  ? 'bg-kaspa-primary text-kaspa-dark'
                  : 'text-gray-400 hover:text-white'
              }`}
            >
              {t(`tournaments.detail.tabs.${tab}`)}
            </button>
          ))}
        </div>

        {/* Tab content */}
        {activeTab === 'overview' && (
          <div className="space-y-4">
            <div className="grid sm:grid-cols-2 gap-4">
              <div className="glass-panel p-6 border border-kaspa-primary/20 rounded-2xl shadow-glow-primary">
                <h3 className="text-sm font-bold text-gray-400 uppercase tracking-wider mb-3">{t('tournaments.detail.overview.info_title')}</h3>
                <dl className="space-y-4 text-sm">
                  <div className="flex justify-between items-center">
                    <dt className="text-gray-500">{t('tournaments.detail.overview.status')}</dt>
                    <dd className="text-white font-semibold">
                      <StatusBadge status={tournament.status} />
                    </dd>
                  </div>
                  <div className="flex justify-between items-center">
                    <dt className="text-gray-500">{t('tournaments.detail.overview.game')}</dt>
                    <dd className="text-white font-semibold">
                      <div className="flex items-center gap-2 bg-white/5 px-2.5 py-1 rounded-xl border border-white/10">
                        {(() => {
                          const game = SUPPORTED_GAMES.find(g => g.id === tournament.game_id);
                          return game?.icon ? (
                            <img src={game.icon} alt={game.name} className="w-5 h-5 object-contain" />
                          ) : null;
                        })()}
                        <span className="uppercase tracking-wider">{tournament.game_id.toUpperCase()}</span>
                      </div>
                    </dd>
                  </div>
                  <div className="flex justify-between items-center pt-2 border-t border-white/5">
                    <dt className="text-gray-500">{t('tournaments.card.max_teams')}</dt>
                    <dd className="text-white font-black">{tournament.max_teams}</dd>
                  </div>
                  <div className="flex justify-between items-center">
                    <dt className="text-gray-500">{t('tournaments.detail.overview.buy_in')}</dt>
                    <dd className="text-emerald-400 font-black">{sompiToKas(tournament.buy_in_sompi)} KAS</dd>
                  </div>
                  <div className="flex justify-between items-center">
                    <dt className="text-gray-500">{t('tournaments.detail.overview.prize_pool')}</dt>
                    <dd className="text-kaspa-primary font-black">{sompiToKas(['COMPLETED', 'BRACKET_READY', 'IN_PROGRESS', 'FUNDED'].includes(tournament.status) ? tournament.total_prize_pool_sompi : tournament.buy_in_sompi * tournament.max_teams)} KAS</dd>
                  </div>

                </dl>
              </div>
              <div className="glass-panel p-6 border border-kaspa-primary/20 rounded-2xl shadow-glow-primary">
                <h3 className="text-sm font-bold text-gray-400 uppercase tracking-wider mb-3">{t('tournaments.detail.overview.escrow_title')}</h3>
                {tournament.escrow_address ? (
                  <div className="space-y-2">
                    <p className="text-xs text-gray-500">{t('tournaments.detail.overview.escrow_address')}</p>
                    <code className="block text-xs text-kaspa-primary break-all bg-kaspa-dark/80 rounded-lg p-3">
                      {tournament.escrow_address}
                    </code>
                    {tournament.payout_tx_hash && (
                      <>
                        <p className="text-xs text-gray-500 mt-3">{t('tournaments.detail.overview.payout_tx')}</p>
                        <code className="block text-xs text-green-400 break-all bg-kaspa-dark/80 rounded-lg p-3">
                          {tournament.payout_tx_hash}
                        </code>
                      </>
                    )}
                  </div>
                ) : (
                  <p className="text-sm text-gray-600">{t('tournaments.detail.overview.no_escrow')}</p>
                )}
              </div>
            </div>
          </div>
        )}

        {activeTab === 'teams' && (() => {
          const myTeamInList = myTeam && teams.some(t => t.id === myTeam.id);
          const showDepositBlock = myTeamInList && ['REGISTRATION', 'FUNDED', 'BRACKET_READY'].includes(tournament.status) && !!tournament.escrow_address;

          return (
          <div>
            <TeamsList teams={teams} myTeamIds={myTeamIds} />
            {showDepositBlock && <DepositBlock tournament={tournament} myTeam={myTeam} onDepositSuccess={load} />}
            {showRegister && (
              <form onSubmit={handleRegister} className="mt-4 glass-panel border border-kaspa-primary/20 rounded-2xl p-6 space-y-3">
                <p className="text-sm font-bold text-blue-300">{t('tournaments.detail.teams.register_title')}</p>
                <input
                  id="team-name-input"
                  className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-kaspa-primary"
                  placeholder={t('tournaments.detail.teams.team_name')}
                  value={teamName}
                  onChange={e => setTeamName(e.target.value)}
                  required minLength={2}
                />
                {regError && <p className="text-red-400 text-xs">{regError}</p>}
                <div className="flex gap-2">
                  <button
                    id="register-submit-btn"
                    type="submit"
                    disabled={regLoading || !teamName.trim()}
                    className="px-5 py-2 bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark font-bold rounded-lg text-sm disabled:opacity-40 transition-all"
                  >
                    {regLoading ? t('tournaments.detail.teams.registering') : t('tournaments.detail.teams.register_btn')}
                  </button>
                  <button
                    type="button"
                    onClick={() => setShowRegister(false)}
                    className="px-4 py-2 bg-white/5 hover:bg-white/10 text-gray-400 rounded-lg text-sm"
                  >
                    {t('tournaments.detail.teams.cancel')}
                  </button>
                </div>
              </form>
            )}
          </div>
          );
        })()}

        {activeTab === 'bracket' && (
          <BracketView
            slots={bracket}
            tournamentId={id!}
            isCaptain={isCaptain}
            myTeamIds={myTeamIds}
            onRefresh={load}
          />
        )}
      </div>
    </div>
  );
}
