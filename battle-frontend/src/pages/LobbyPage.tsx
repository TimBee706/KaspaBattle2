import React, { useEffect } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useAuthStore } from '../stores/useAuthStore';
import { useLobbies } from '../hooks/useLobbies';
import { LobbyTable } from '../components/LobbyTable';
import { ChallengeModal } from '../components/ChallengeModal';

export const LobbyPage: React.FC = () => {
    useLobbies();
    const { lobbies, setLobbies, addOrUpdateLobby } = useLobbyStore();
    const { user } = useAuthStore();
    const kaspaAddress = user?.kas_address || null;
    const faceitId = user?.faceit_id || null;

    useEffect(() => {
        fetch('/api/lobbies').then(r => r.json()).then(data => setLobbies(data || [])).catch(console.error);
        const ws = new WebSocket(`ws://${window.location.host}/ws`);
        ws.onmessage = e => { try { addOrUpdateLobby(JSON.parse(e.data)); } catch (err) { } };
        return () => ws.close();
    }, [setLobbies, addOrUpdateLobby]);

    const isMyLobby = (l: any) => l.player_a_kas_address === kaspaAddress || l.player_b_kas_address === kaspaAddress || l.player_a_faceit_id === faceitId || l.player_b_faceit_id === faceitId;
    const myLobbies = lobbies.filter(l => kaspaAddress || faceitId ? isMyLobby(l) : false);
    const openLobbies = lobbies.filter(l => l.status === 'OPEN' && !isMyLobby(l));

    return (
        <div className="container mx-auto p-4 max-w-5xl">
            <div className="flex justify-between items-center mb-8">
                <h1 className="text-3xl font-bold text-white">Lobby</h1>
                <ChallengeModal />
            </div>
            <LobbyTable title="Meine Lobbys" matches={myLobbies} isMyLobbies={true} />
            <LobbyTable title="Offene Lobbys" matches={openLobbies} isMyLobbies={false} />
        </div>
    );
};
