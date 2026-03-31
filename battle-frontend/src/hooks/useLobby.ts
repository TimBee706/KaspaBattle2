import { useState, useCallback, useMemo } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useWalletStore } from '../stores/useWalletStore';
import { FEATURE_FLAGS } from '../config/featureFlags';
import { SUPPORTED_GAMES } from '../config/constants';
import apiClient from '../api/client';

export function useLobby() {
    const { lobbies, addOrUpdateLobby } = useLobbyStore();
    const { isConnected, balanceSompi, address } = useWalletStore();

    const [selectedLobbyId, setSelectedLobbyId] = useState<string | null>(null);
    const [isCreating, setIsCreating] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const selectedLobby = useMemo(() =>
        lobbies.find(l => l.id === selectedLobbyId) || null
        , [lobbies, selectedLobbyId]);

    const handleLobbyClick = useCallback((id: string) => {
        setSelectedLobbyId(id);
    }, []);

    const closeDetail = useCallback(() => {
        setSelectedLobbyId(null);
    }, []);

    const MIN_WAGER_KAS = 10; // Minimum requirement to create a challenge

    const canCreateChallenge = useMemo(() => {
        const balanceKas = balanceSompi / 100_000_000;
        return isConnected && balanceKas >= MIN_WAGER_KAS;
    }, [isConnected, balanceSompi]);

    const createChallenge = useCallback(async (data: {
        stakeKas: number,
        mode: 'BO1' | 'BO3',
        gameId?: string,
    }) => {
        if (!canCreateChallenge) throw new Error("Voraussetzungen nicht erfüllt");
        if (!address) throw new Error("Wallet nicht verbunden");

        setIsCreating(true);
        setError(null);
        try {
            // STEP 1 - TEST MODE: Direct creation, no escrow signature needed yet
            if (FEATURE_FLAGS.TEST_MODE) {
                // Get escrow address from wallet account (generated at index 1)
                const { account } = useWalletStore.getState();
                const escrowAddr = (account as any)?.escrowAddress || address;

                const response = await apiClient.post('/challenges', {
                    game_id: data.gameId || SUPPORTED_GAMES[0].id,
                    wager_sompi: Math.round(data.stakeKas * 100_000_000),
                    mode: data.mode,
                    escrow_address: escrowAddr,
                });
                if (response.data) addOrUpdateLobby(response.data);
                return response.data;
            } else {
                // STEP 2 - PRODUCTION: Sign TX with Kaspa wallet for escrow
                // [TBD in Step 2 branch]
                throw new Error("Produktions-Modus (Escrow) ist noch nicht implementiert.");
            }
        } catch (err: any) {
            const msg = err.response?.data?.error || err.message || "Fehler beim Erstellen der Challenge";
            setError(msg);
            throw new Error(msg);
        } finally {
            setIsCreating(false);
        }
    }, [addOrUpdateLobby, canCreateChallenge, address]);

    return {
        selectedLobby,
        isDetailOpen: !!selectedLobby,
        isCreating,
        error,
        canCreateChallenge,
        minWager: MIN_WAGER_KAS,
        handleLobbyClick,
        closeDetail,
        createChallenge
    };
}
