import React, { useEffect } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useAuthStore } from '../stores/useAuthStore';
import { useNavigate } from 'react-router-dom';
import { LobbyTable } from '../components/LobbyTable';
import { useParams } from 'react-router-dom';
import apiClient from '../api/client';
import { WS_BASE_URL } from '../config/constants';
import { useTranslation } from 'react-i18next';
import { isAvailableChallenge, isMyLobby } from '../domain/lobby';

export const LobbyPage: React.FC = () => {
    const navigate = useNavigate();
    const { lobbies, setLobbies, addOrUpdateLobby } = useLobbyStore();
    const { user, fetchUser, testMode, setTestMode } = useAuthStore();
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
        ws.onmessage = e => { try { addOrUpdateLobby(JSON.parse(e.data)); } catch (err) { } };
        return () => ws.close();
    }, [setLobbies, addOrUpdateLobby, fetchUser, setTestMode]);

    // myLobbies: matches where the current user is creator or opponent
    // NOTE: backend returns creator_user_id/opponent_user_id, NOT player_a_faceit_id
    const currentUserId = user?.id ?? null;
    const myLobbies = lobbies.filter((lobby) => isMyLobby(lobby, currentUserId));
    const openLobbies = lobbies.filter((lobby) => isAvailableChallenge(lobby, currentUserId));

    return (
        <div className="container mx-auto px-4 py-8">
            <div className="flex justify-between items-center mb-12">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('lobby.title')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('lobby.active_matches')}</p>
                </div>
                <button
                    onClick={() => navigate('/lobby/create')}
                    className="bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-8 py-3 rounded-xl font-black uppercase tracking-tighter transition-all shadow-xl shadow-kaspa-primary/10 active:scale-95"
                >
                    {t('lobby.create_challenge')}
                </button>
            </div>

            {testMode && <div className="p-4 bg-blue-900/20 border border-blue-500/20 rounded-xl text-blue-400 text-xs font-black uppercase tracking-widest mb-10 flex items-center gap-3">
                <span className="bg-blue-500/20 p-2 rounded-lg animate-pulse">🧪</span>
                {t('lobby.test_mode')}
            </div>}

            <LobbyTable
                matches={myLobbies}
                title={t('lobby.my_lobbies')}
                isMyLobbies={true}
                onLobbyClick={(id) => navigate(`/match/${id}`)}
            />
            <LobbyTable
                matches={openLobbies}
                title={t('lobby.available_challenges')}
                onLobbyClick={(id) => navigate(`/match/${id}`)}
            />
        </div>
    );
};
