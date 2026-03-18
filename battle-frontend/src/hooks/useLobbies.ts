import { useEffect } from 'react';
import { API_BASE_URL } from '../config/constants';
import { useLobbyStore } from '../stores/useLobbyStore';

export const useLobbies = () => {
    const { setLobbies, addOrUpdateLobby } = useLobbyStore();

    useEffect(() => {
        fetch(`${API_BASE_URL}/lobbies`)
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
