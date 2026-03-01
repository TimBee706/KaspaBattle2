import React, { useEffect } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useAuthStore } from '../stores/useAuthStore';
import { LobbyTable } from '../components/LobbyTable';
import { ChallengeModal } from '../components/ChallengeModal';
import apiClient from '../api/client';
import { WS_BASE_URL } from '../config/constants';
import { useTranslation } from 'react-i18next';

export const LobbyPage: React.FC = () => {
    const { lobbies, setLobbies, addOrUpdateLobby } = useLobbyStore();
    const { user, fetchUser, testMode, setTestMode } = useAuthStore();
    const { t } = useTranslation();

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

    const myLobbies = lobbies.filter(m =>
        (user?.faceit_id && (m.player_a_faceit_id === user.faceit_id || m.player_b_faceit_id === user.faceit_id)) ||
        (user?.id && (m.creator_user_id === user.id || m.opponent_user_id === user.id))
    );
    const openLobbies = lobbies.filter(m => m.status === 'OPEN' && !myLobbies.includes(m));

    return (
        <div className="container mx-auto px-4 py-8">
            <div className="flex justify-between items-center mb-8">
                <h1 className="text-3xl font-black text-white uppercase tracking-tighter">{t('lobby.title')}</h1>
                <ChallengeModal />
            </div>

            {testMode && <div className="p-3 bg-blue-900/20 border border-blue-500/30 rounded-lg text-blue-400 text-sm mb-6 flex items-center gap-2">
                <span className="animate-pulse">🧪</span>
                {t('lobby.test_mode')}
            </div>}

            <LobbyTable matches={myLobbies} title={t('lobby.my_lobbies')} isMyLobbies={true} />
            <LobbyTable matches={openLobbies} title={t('lobby.available_challenges')} />
        </div>
    );
};
