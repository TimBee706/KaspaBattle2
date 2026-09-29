import React, { useState } from 'react';
import { type BattleMatch, getMatchMode, getMatchStakeSompi, getMatchCreatedAt, getMatchProvider } from '../api/types';
import { useAuthStore } from '../stores/useAuthStore';
import apiClient from '../api/client';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { MatchStatusBadge } from './match/MatchStatusBadge';
import { Icon } from './Icon';
import { startFaceitLogin } from '../api/auth';
import { ProviderBadge } from './match/ProviderBadge';
import { useAccess } from '../hooks/useAccess';
import { canPlayMatch } from '../domain/access';

export const LobbyTable: React.FC<{ matches: BattleMatch[], title?: string, onLobbyClick?: (id: string) => void }> = ({ matches, title, onLobbyClick }) => {
    const { user, testMode, isFaceitConnected } = useAuthStore();
    const kaspaAddress = user?.kaspa_address || null;
    const access = useAccess();
    const [loading, setLoading] = useState<string | null>(null);
    const navigate = useNavigate();
    const { t } = useTranslation();

    const join = async (id: string) => {
        setLoading(id);
        try {
            await apiClient.post(`/matches/${id}/accept`);
            navigate(`/escrow/${id}`);
        } catch (e) { console.error(e); }
        finally { setLoading(null); }
    };

    return (
        <div className="mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/15">
            {title && <h2 className="text-xl font-black mb-6 text-white uppercase tracking-tighter pl-2">{title}</h2>}
            <div className="flex flex-col gap-3">
                {matches.length === 0 && (
                    <div className="flex flex-col items-center text-center py-12 px-4">
                        <div className="w-12 h-12 rounded-xl bg-white/5 border border-white/10 flex items-center justify-center mb-4">
                            <Icon name="search" className="w-5 h-5 text-gray-500" />
                        </div>
                        <p className="text-lg font-semibold text-gray-400">{t('lobby.empty.title')}</p>
                        <p className="text-sm text-gray-500 mt-1">
                            {(isFaceitConnected || testMode) ? t('lobby.empty.subtitle_auth') : t('lobby.empty.subtitle_guest')}
                        </p>
                        <button
                            onClick={() => ((isFaceitConnected || testMode) ? navigate('/lobby/create') : startFaceitLogin())}
                            className="mt-6 bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-6 py-2.5 rounded-lg font-black uppercase tracking-tighter transition-all shadow-glow-subtle active:scale-95"
                        >
                            {(isFaceitConnected || testMode) ? t('lobby.create_challenge') : t('lobby.login_create_btn')}
                        </button>
                    </div>
                )}
                {matches.map((m, index) => {
                    const isNative = getMatchProvider(m) === 'NATIVE';
                    const matchTitle = isNative
                        ? `${m.player_a_display_name || t('match.challenger')} vs ${m.player_b_display_name || 'TBD'}`
                        : `${m.player_a_faceit_nickname} vs ${m.player_b_faceit_nickname || 'TBD'}`;
                    const createdAt = getMatchCreatedAt(m);

                    return (
                        <div
                            key={`${m.id}-${index}`}
                            onClick={() => onLobbyClick?.(m.id)}
                            role="button"
                            tabIndex={0}
                            onKeyDown={(e) => e.key === 'Enter' && onLobbyClick?.(m.id)}
                            className="group bg-kaspa-card/60 border border-kaspa-border p-4 rounded-xl flex flex-col md:flex-row justify-between items-start md:items-center gap-4 hover:border-kaspa-primary/40 hover:bg-kaspa-card transition-all cursor-pointer select-none active:scale-[0.99]"
                        >
                            {/* Left side */}
                            <div className="flex-1 min-w-0">
                                <div className="flex items-center gap-3 mb-1.5">
                                    <h3 className="font-bold text-white text-lg truncate group-hover:text-kaspa-primary transition-colors">
                                        {matchTitle}
                                    </h3>
                                    <MatchStatusBadge status={m.status} />
                                </div>
                                <div className="flex flex-wrap items-center gap-3 text-2xs text-gray-500 font-bold">
                                    <ProviderBadge match={m} />
                                    {!isNative && (
                                        <>
                                            <span className="text-gray-700">•</span>
                                            <span className="uppercase tracking-wider">{getMatchMode(m)}</span>
                                        </>
                                    )}
                                    {createdAt && (
                                        <>
                                            <span className="text-gray-700">•</span>
                                            <span>{new Date(createdAt).toLocaleString()}</span>
                                        </>
                                    )}
                                </div>
                            </div>

                            {/* Right side */}
                            <div className="flex items-center gap-4 md:gap-6 w-full md:w-auto mt-2 md:mt-0 pt-3 md:pt-0 border-t md:border-0 border-kaspa-border/60">
                                <div className="text-right shrink-0">
                                    <div className="text-kaspa-primary text-xl font-black flex items-baseline gap-1 justify-end">
                                        {(getMatchStakeSompi(m) / 100_000_000).toLocaleString('de-DE', { minimumFractionDigits: 2 })} <span className="text-sm font-bold text-kaspa-primary/80">KAS</span>
                                    </div>
                                    <div className="text-2xs text-gray-500 font-bold uppercase tracking-widest mt-0.5">
                                        {t('match.stake_per_player')}
                                    </div>
                                </div>
                                {m.creator_user_id !== user?.id && m.opponent_user_id !== user?.id && m.status === 'OPEN' && !m.opponent_user_id && (
                                    <button
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            join(m.id);
                                        }}
                                        disabled={loading === m.id || !kaspaAddress || (isNative ? !canPlayMatch(access, m) : (!isFaceitConnected && !testMode))}
                                        className="bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-6 py-2.5 rounded-lg font-black uppercase tracking-tighter disabled:opacity-30 disabled:hover:bg-kaspa-primary transition-all shadow-glow-subtle active:scale-95"
                                    >
                                        {loading === m.id ? '...' : t('lobby.join')}
                                    </button>
                                )}
                            </div>
                        </div>
                    );
                })}
            </div>

            {(!kaspaAddress || testMode || (kaspaAddress && isFaceitConnected) || (kaspaAddress && !isFaceitConnected)) && (
                <div className="mt-4 pt-4 border-t border-kaspa-border/60">
                    {!kaspaAddress && (
                        <p className="text-red-400 text-sm font-semibold flex items-center gap-2">
                            <Icon name="alert-triangle" className="w-4 h-4 shrink-0" />
                            {t('lobby.connect_wallet_join')}
                        </p>
                    )}
                    {kaspaAddress && isFaceitConnected && !testMode && (
                        <p className="text-emerald-400 text-sm font-semibold flex items-center gap-2">
                            <span className="inline-block w-2 h-2 bg-emerald-500 rounded-full shrink-0" />
                            {t('lobby.faceit_connected_as')} <strong className="text-white">{user?.faceit_nickname}</strong>
                        </p>
                    )}
                    {kaspaAddress && !isFaceitConnected && !testMode && (
                        <p className="text-amber-400 text-sm font-semibold flex items-center gap-2">
                            <Icon name="alert-triangle" className="w-4 h-4 shrink-0" />
                            {t('lobby.connect_faceit_join')}
                        </p>
                    )}
                    {testMode && (
                        <p className="text-kaspa-primary/90 text-sm font-medium italic flex items-center gap-2">
                            <Icon name="beaker" className="w-4 h-4 shrink-0" />
                            {t('lobby.test_mode_notice')}
                        </p>
                    )}
                </div>
            )}
        </div>
    );
};
