import React, { useEffect, useState } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useAuthStore } from '../stores/useAuthStore';
import { useNavigate } from 'react-router-dom';
import { LobbyTable } from '../components/LobbyTable';
import { useParams } from 'react-router-dom';
import apiClient from '../api/client';
import { WS_BASE_URL } from '../config/constants';
import { useTranslation } from 'react-i18next';

import { Icon } from '../components/Icon';

export const LobbyPage: React.FC = () => {
    const navigate = useNavigate();
    const [filter, setFilter] = useState<'ALL' | 'OPEN' | 'LIVE' | 'COMPLETED'>('ALL');
    const { lobbies, setLobbies, addOrUpdateLobby } = useLobbyStore();
    const { fetchUser, testMode, setTestMode } = useAuthStore();
    const { t } = useTranslation();
    const { lobbyId } = useParams<{ lobbyId: string }>();

    // Support deep linking
    useEffect(() => {
        if (lobbyId) {
            navigate(`/match/${lobbyId}`);
        }
    }, [lobbyId, navigate]);

    useEffect(() => {
        fetchUser().then(() => {
            if (useAuthStore.getState().user?.display_name === "TestUser") {
                setTestMode(true);
            }
        }).catch(console.error);

        apiClient.get('/lobbies').then(res => setLobbies(res.data || [])).catch(console.error);
        const ws = new WebSocket(WS_BASE_URL);
        ws.onmessage = (e) => {
            try {
                const data = JSON.parse(e.data);
                if (data && data.id && 'player_a_wallet' in data) {
                    addOrUpdateLobby(data);
                }
            } catch {
                console.warn('[LobbyPage] Ignoring invalid lobby websocket payload');
            }
        };
        return () => ws.close();
    }, [setLobbies, addOrUpdateLobby, fetchUser, setTestMode]);

    // myLobbies: matches where the current user is creator or opponent
    // NOTE: backend returns creator_user_id/opponent_user_id, NOT player_a_faceit_id
    
    
    const filteredMatches = lobbies.filter(lobby => {
        if (filter === 'ALL') return true;
        if (filter === 'OPEN') return ['DRAFT', 'OPEN', 'WAITING_FOR_DEPOSITS', 'AWAITING_FUNDING'].includes(lobby.status);
        if (filter === 'LIVE') return ['FUNDED', 'LOCKED', 'GAME_ID_INPUT', 'IN_GAME', 'READY_FOR_PAYOUT', 'RESOLVING', 'DISPUTED'].includes(lobby.status);
        if (filter === 'COMPLETED') return ['FINISHED_FACEIT', 'RESOLVED', 'PAID_OUT', 'CANCELLED', 'REFUNDED'].includes(lobby.status);
        return true;
    });

    const filterOptions: Array<{ value: 'ALL' | 'OPEN' | 'LIVE' | 'COMPLETED'; label: string }> = [
        { value: 'ALL', label: t('tournaments.filter.all') },
        { value: 'OPEN', label: t('tournaments.filter.open') },
        { value: 'LIVE', label: t('tournaments.filter.live') },
        { value: 'COMPLETED', label: t('tournaments.filter.completed') },
    ];

    return (
        <div className="container mx-auto px-4 py-8">
            <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('lobby.title')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('lobby.active_matches')}</p>
                </div>
                <div className="flex items-center gap-3 w-full md:w-auto">
                    <button
                        onClick={() => apiClient.get('/lobbies').then(res => setLobbies(res.data || [])).catch(console.error)}
                        className="p-2 rounded-lg border border-white/10 hover:border-kaspa-primary/30 text-gray-400 hover:text-kaspa-primary transition-all shrink-0"
                        title={t('lobby.refresh')}
                    >
                        ↻
                    </button>
                    <button
                        onClick={() => navigate('/lobby/create')}
                        className="w-full md:w-auto bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-8 py-3 rounded-xl font-black uppercase tracking-tighter transition-all shadow-xl shadow-kaspa-primary/10 active:scale-95 text-center"
                    >
                        {t('lobby.create_challenge')}
                    </button>
                </div>
            </div>

            {testMode && <div className="glass-panel p-4 border-blue-500/20 text-blue-400 text-xs font-black uppercase tracking-widest mb-10 flex items-center gap-3">
                <Icon name="beaker" className="w-4 h-4 shrink-0 text-blue-400" />
                {t('lobby.test_mode')}
            </div>}

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

            <LobbyTable
                matches={filteredMatches}
                onLobbyClick={(id) => navigate(`/match/${id}`)}
            />
        </div>
    );
};
