import { useEffect } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';

export const useLobbies = () => {
    const { setLobbies, addOrUpdateLobby } = useLobbyStore();

    useEffect(() => {
        fetch('/api/lobbies')
            .then(res => res.json())
            .then(data => setLobbies(data || []))
            .catch(console.error);

        const ws = new WebSocket(`ws://${window.location.host}/ws`);
        ws.onmessage = (event) => {
            try { addOrUpdateLobby(JSON.parse(event.data)); } catch (e) { }
        };
        return () => ws.close();
    }, [setLobbies, addOrUpdateLobby]);
};
