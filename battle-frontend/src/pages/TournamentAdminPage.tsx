import { useState, useEffect, useCallback } from 'react';
import {
  listTournaments,
  adminResolveBracketDispute,
  adminTriggerPayout,
  adminCancelTournament,
  getPayoutInfo,
  listTeams,
  getBracket,
  sompiToKas,
  STATUS_LABELS,
  STATUS_COLORS,
  type Tournament,
  type TournamentStatus,
  type PayoutInfo,
} from '../api/tournaments';
import { useAuthStore } from '../stores/useAuthStore';
import { useTranslation } from 'react-i18next';

// ─── Status badge ─────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: TournamentStatus }) {
  const color = STATUS_COLORS[status] ?? '#6b7280';
  return (
    <span
      style={{ color, borderColor: color + '44', backgroundColor: color + '18' }}
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-semibold border"
    >
      <span style={{ backgroundColor: color }} className="w-1.5 h-1.5 rounded-full" />
      {STATUS_LABELS[status] ?? status}
    </span>
  );
}

// ─── Tournament Admin Row ─────────────────────────────────────────────────────

interface AdminRowProps {
  tournament: Tournament;
  expanded: boolean;
  onToggle: () => void;
  onRefresh: () => void;
}

function TournamentAdminRow({ tournament, expanded, onToggle, onRefresh }: AdminRowProps) {
  const { t } = useTranslation();
  const [payout, setPayout] = useState<PayoutInfo | null>(null);
  const [disputes, setDisputes] = useState<{ id: string; round: number; slot_index: number; reason?: string; team_a_id?: string; team_b_id?: string; winner_team_id?: string }[]>([]);
  const [resolveTeamId, setResolveTeamId] = useState<Record<string, string>>({});
  const [payoutLoading, setPayoutLoading] = useState(false);
  const [cancelLoading, setCancelLoading] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!expanded) return;
    const loadDetails = async () => {
      setLoading(true);
      try {
        const [p, bracket] = await Promise.all([
          getPayoutInfo(tournament.id),
          getBracket(tournament.id),
        ]);
        setPayout(p);
        setDisputes(bracket.filter(s => s.disputed) as typeof disputes);
      } catch { /* ignore */ }
      setLoading(false);
    };
    loadDetails();
  }, [expanded, tournament.id]);

  const handleTriggerPayout = async () => {
    if (!confirm(t('tournaments.admin.payout.confirm_trigger'))) return;
    setPayoutLoading(true);
    try {
      await adminTriggerPayout(tournament.id);
      onRefresh();
    } catch { /* ignore */ } finally { setPayoutLoading(false); }
  };

  const handleCancel = async () => {
    if (!confirm(t('tournaments.admin.cancel.confirm_cancel'))) return;
    setCancelLoading(true);
    try {
      await adminCancelTournament(tournament.id);
      onRefresh();
    } catch { /* ignore */ } finally { setCancelLoading(false); }
  };

  const handleResolveDispute = async (slotId: string, teamId: string) => {
    if (!confirm(t('tournaments.admin.disputes.confirm_resolve'))) return;
    try {
      await adminResolveBracketDispute(tournament.id, slotId, teamId);
      onRefresh();
    } catch { /* ignore */ }
  };

  return (
    <div className="bg-[#0d1b2a] border border-white/5 rounded-xl overflow-hidden">
      {/* Header row */}
      <button
        id={`admin-row-${tournament.id}`}
        onClick={onToggle}
        className="w-full flex items-center justify-between p-4 hover:bg-white/2 transition-colors text-left"
      >
        <div className="flex items-center gap-3 min-w-0">
          <StatusBadge status={tournament.status} />
          <span className="font-semibold text-white truncate">{tournament.name}</span>
          {tournament.status === 'DISPUTED' && (
            <span className="text-orange-400 text-xs font-bold animate-pulse">⚠️ DISPUTED</span>
          )}
        </div>
        <div className="flex items-center gap-4 text-sm text-gray-400 shrink-0">
          <span>{sompiToKas(tournament.total_prize_pool_sompi)} KAS</span>
          <span className={`transition-transform ${expanded ? 'rotate-180' : ''}`}>▼</span>
        </div>
      </button>

      {/* Expanded panel */}
      {expanded && (
        <div className="border-t border-white/5 p-4 space-y-4">
          {loading ? (
            <div className="text-center py-4 text-gray-500 text-sm">{t('tournaments.admin.loading')}</div>
          ) : (
            <>
              {/* Payout info */}
              {payout && (
                <div className="bg-[#0a0f14] rounded-xl p-4 space-y-2">
                  <h4 className="text-xs font-bold text-gray-400 uppercase tracking-wider mb-3">{t('tournaments.admin.payout.title')}</h4>
                  <div className="grid grid-cols-2 gap-2 text-sm">
                    {[
                      [t('tournaments.admin.payout.winner_share'), `${sompiToKas(payout.winner_share_sompi)} KAS`],
                      [t('tournaments.admin.payout.runner_up_share'), `${sompiToKas(payout.runner_up_share_sompi)} KAS`],
                      [t('tournaments.admin.payout.platform_fee'), `${sompiToKas(payout.platform_fee_sompi)} KAS`],
                      [t('tournaments.admin.payout.winner_address'), payout.winner_kaspa_address ?? t('tournaments.admin.payout.not_set')],
                    ].map(([k, v]) => (
                      <div key={k} className="flex justify-between gap-2">
                        <span className="text-gray-500">{k}</span>
                        <span className="text-white font-semibold truncate max-w-[160px]" title={v}>{v}</span>
                      </div>
                    ))}
                  </div>
                  {payout.payout_tx_hash ? (
                    <div className="mt-2 bg-[#49EACB]/5 border border-[#49EACB]/20 rounded-lg p-3">
                      <p className="text-xs text-gray-500 mb-1">{t('tournaments.admin.payout.payout_tx')}</p>
                      <code className="text-xs text-[#49EACB] break-all">{payout.payout_tx_hash}</code>
                    </div>
                  ) : tournament.status === 'COMPLETED' && (
                    <button
                      id={`trigger-payout-${tournament.id}`}
                      onClick={handleTriggerPayout}
                      disabled={payoutLoading}
                      className="mt-2 w-full py-2 bg-[#49EACB] hover:bg-[#3dd4b8] text-[#070d14] font-bold rounded-lg text-sm disabled:opacity-40 transition-all"
                    >
                      {payoutLoading ? t('tournaments.admin.payout.triggering') : t('tournaments.admin.payout.trigger_btn')}
                    </button>
                  )}
                </div>
              )}

              {/* Disputed bracket slots */}
              {disputes.length > 0 && (
                <div className="bg-orange-950/20 border border-orange-500/20 rounded-xl p-4">
                  <h4 className="text-xs font-bold text-orange-400 uppercase tracking-wider mb-3">
                    {t('tournaments.admin.disputes.title', { count: disputes.length })}
                  </h4>
                  <div className="space-y-3">
                    {disputes.map(slot => (
                      <div key={slot.id} className="bg-[#0a0f14] rounded-lg p-3">
                        <div className="text-sm text-white mb-1">
                          {t('tournaments.admin.disputes.round_slot', { round: slot.round + 1, slot: slot.slot_index + 1 })}
                        </div>
                        {slot.reason && (
                          <p className="text-xs text-gray-400 mb-2 italic">&ldquo;{slot.reason}&rdquo;</p>
                        )}
                        {!slot.winner_team_id && (
                          <div className="flex gap-2">
                            {[slot.team_a_id, slot.team_b_id].filter(Boolean).map(tid => (
                              <div key={tid} className="flex items-center gap-1.5">
                                <select
                                  id={`resolve-select-${slot.id}-${tid}`}
                                  className="text-xs bg-[#1a2332] border border-orange-500/30 rounded px-2 py-1 text-white"
                                  value={resolveTeamId[slot.id] ?? ''}
                                  onChange={e => setResolveTeamId(prev => ({ ...prev, [slot.id]: e.target.value }))}
                                >
                                  <option value="">{t('tournaments.admin.disputes.select_winner')}</option>
                                  {[slot.team_a_id, slot.team_b_id].filter(Boolean).map(t => (
                                    <option key={t} value={t!}>{t!.slice(0, 8)}…</option>
                                  ))}
                                </select>
                                <button
                                  id={`resolve-dispute-${slot.id}`}
                                  onClick={() => resolveTeamId[slot.id] && handleResolveDispute(slot.id, resolveTeamId[slot.id])}
                                  disabled={!resolveTeamId[slot.id]}
                                  className="text-xs py-1 px-2 bg-orange-600 hover:bg-orange-500 text-white rounded disabled:opacity-40 transition-all"
                                >
                                  {t('tournaments.admin.disputes.resolve_btn')}
                                </button>
                              </div>
                            ))}
                          </div>
                        )}
                        {slot.winner_team_id && (
                          <span className="text-xs text-[#49EACB]">{t('tournaments.admin.disputes.resolved')}</span>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Admin actions */}
              <div className="flex gap-2 flex-wrap">
                {!['COMPLETED', 'CANCELLED'].includes(tournament.status) && (
                  <button
                    id={`admin-cancel-${tournament.id}`}
                    onClick={handleCancel}
                    disabled={cancelLoading}
                    className="px-4 py-2 bg-red-600/20 hover:bg-red-600/30 border border-red-500/30 text-red-400 text-sm font-semibold rounded-lg disabled:opacity-40 transition-all"
                  >
                    {cancelLoading ? t('tournaments.admin.cancel.cancelling') : t('tournaments.admin.cancel.btn')}
                  </button>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

// ─── Stats Bar ────────────────────────────────────────────────────────────────

function StatsBar({ tournaments }: { tournaments: Tournament[] }) {
  const { t } = useTranslation();
  const stats = [
    { label: t('tournaments.admin.stats.total'), value: tournaments.length, color: '#49EACB' },
    { label: t('tournaments.admin.stats.live'), value: tournaments.filter(t => t.status === 'IN_PROGRESS').length, color: '#10b981' },
    { label: t('tournaments.admin.stats.disputed'), value: tournaments.filter(t => t.status === 'DISPUTED').length, color: '#f97316' },
    { label: t('tournaments.admin.stats.completed'), value: tournaments.filter(t => t.status === 'COMPLETED').length, color: '#6b7280' },
    {
      label: t('tournaments.admin.stats.prize_pool'),
      value: `${sompiToKas(tournaments.reduce((s, t) => s + t.total_prize_pool_sompi, 0))} KAS`,
      color: '#fbbf24',
    },
  ];

  return (
    <div className="grid grid-cols-2 sm:grid-cols-5 gap-3">
      {stats.map(s => (
        <div key={s.label} className="bg-[#0d1b2a] border border-white/5 rounded-xl p-4 text-center">
          <div style={{ color: s.color }} className="text-2xl font-black">{s.value}</div>
          <div className="text-gray-500 text-xs mt-1">{s.label}</div>
        </div>
      ))}
    </div>
  );
}

// ─── Main Admin Page ──────────────────────────────────────────────────────────

export function TournamentAdminPage() {
  const { t } = useTranslation();
  const user = useAuthStore(s => s.user);
  const [tournaments, setTournaments] = useState<Tournament[]>([]);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filter, setFilter] = useState<TournamentStatus | 'ALL'>('ALL');

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listTournaments();
      setTournaments(data.sort((a, b) => {
        // Sort: disputed first, then in_progress, then by created
        const priority: Record<string, number> = { DISPUTED: 0, IN_PROGRESS: 1, BRACKET_READY: 2, FUNDED: 3, REGISTRATION: 4, COMPLETED: 5, CANCELLED: 6, DRAFT: 7 };
        return (priority[a.status] ?? 9) - (priority[b.status] ?? 9);
      }));
    } catch { /* ignore */ } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  // Super basic admin guard — real guard would be on backend
  if (!user) {
    return (
      <div className="min-h-screen bg-[#070d14] flex items-center justify-center">
        <div className="text-center">
          <div className="text-5xl mb-4">🔒</div>
          <h2 className="text-xl font-bold text-white mb-2">{t('tournaments.admin.auth_required')}</h2>
          <p className="text-gray-500 text-sm">{t('tournaments.admin.auth_msg')}</p>
        </div>
      </div>
    );
  }

  const filtered = filter === 'ALL' ? tournaments : tournaments.filter(t => t.status === filter);

  const filterOptions: Array<{ value: TournamentStatus | 'ALL'; label: string }> = [
    { value: 'ALL', label: t('tournaments.filter.all') },
    { value: 'DISPUTED', label: t('tournaments.filter.disputed') },
    { value: 'IN_PROGRESS', label: t('tournaments.filter.live') },
    { value: 'COMPLETED', label: t('tournaments.filter.completed') },
    { value: 'CANCELLED', label: t('tournaments.filter.cancelled') },
  ];

  return (
    <div className="min-h-screen bg-[#070d14] py-8 px-4">
      <div className="max-w-4xl mx-auto space-y-6">

        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-black text-white">
               {t('tournaments.admin.title').replace('Tournament Admin', '').trim() === '' ? (
                 <>Tournament <span className="text-[#49EACB]">Admin</span></>
               ) : (
                 t('tournaments.admin.title').split(' ').map((word, i) => i === 0 ? <span key={i} className="text-[#49EACB]">{word} </span> : word + ' ')
               )}
            </h1>
            <p className="text-gray-400 text-sm mt-0.5">{t('tournaments.admin.subtitle')}</p>
          </div>
          <button
            id="admin-refresh-btn"
            onClick={load}
            className="p-2 rounded-lg border border-white/10 hover:border-[#49EACB]/30 text-gray-400 hover:text-[#49EACB] transition-all"
          >
            ↻
          </button>
        </div>

        {/* Stats */}
        <StatsBar tournaments={tournaments} />

        {/* Filter */}
        <div className="flex gap-2 flex-wrap">
          {filterOptions.map(opt => (
            <button
              key={opt.value}
              id={`admin-filter-${opt.value.toLowerCase()}`}
              onClick={() => setFilter(opt.value)}
              className={`px-4 py-1.5 rounded-full text-sm font-semibold transition-all ${
                filter === opt.value
                  ? 'bg-[#49EACB] text-[#070d14]'
                  : 'bg-white/5 text-gray-400 hover:text-white hover:bg-white/10'
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>

        {/* List */}
        {loading ? (
          <div className="space-y-3">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="bg-[#0d1b2a] rounded-xl h-16 animate-pulse" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <div className="text-center py-16 text-gray-500">
            <div className="text-4xl mb-3">📋</div>
            <p>{t('tournaments.empty.admin_title')}</p>
          </div>
        ) : (
          <div className="space-y-3">
            {filtered.map(t => (
              <TournamentAdminRow
                key={t.id}
                tournament={t}
                expanded={expanded === t.id}
                onToggle={() => setExpanded(prev => prev === t.id ? null : t.id)}
                onRefresh={load}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
