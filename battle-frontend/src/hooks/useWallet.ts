import { useCallback, useEffect, useRef } from 'react';
import { signMessage } from 'kaspa-wasm';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useMatchStore } from '../stores/useMatchStore';
import { importWallet, getBalanceByAddress, getRpcClient } from '../kaspa/wallet';
import apiClient from '../api/client';

const LEGACY_WALLET_SESSION_KEY = 'kaspa_wallet_session';
const LEGACY_WALLET_PHRASE_KEY = 'kaspa_encrypted_phrase';

interface WalletChallengeResponse {
    challenge_id: string;
    message: string;
    expires_at: string;
}

export function useWallet() {
    const {
        address,
        isConnected,
        isConnecting,
        balanceSompi,
        isFetchingBalance,
        balanceError,
        error,
        setWalletConnection,
        setBalance,
        setFetchingBalance,
        setBalanceError,
        setConnecting,
        setError,
        disconnect: storeDisconnect,
    } = useWalletStore();

    const { updateKasAddress, clearKasAddress, setWalletConnected, fetchUser, isAuthenticated } = useAuthStore();
    const subscriptionActive = useRef(false);

    const fetchBalance = useCallback(async (addr: string) => {
        if (!addr) return;
        setFetchingBalance(true);
        try {
            const sompi = await getBalanceByAddress(addr);
            setBalance(sompi);
        } catch (e: any) {
            setBalanceError(e.message || '--');
        }
    }, [setBalance, setFetchingBalance, setBalanceError]);

    const subscribeToUpdates = useCallback(async (addr: string) => {
        if (subscriptionActive.current) return;
        try {
            const rpc = await getRpcClient();
            await rpc.subscribeUtxosChanged([addr]);

            const handleUtxoChanged = () => {
                console.log('[useWallet] UTXO changed, re-fetching balance...');
                fetchBalance(addr);
            };

            (rpc as any).addEventListener('utxos-changed', handleUtxoChanged);
            subscriptionActive.current = true;
        } catch (e) {
            console.error('[useWallet] Subscription failed:', e);
        }
    }, [fetchBalance]);

    const restoreFullWalletState = useCallback(async () => {
        localStorage.removeItem(LEGACY_WALLET_SESSION_KEY);
        sessionStorage.removeItem(LEGACY_WALLET_PHRASE_KEY);
    }, []);

    const connectWithMnemonic = useCallback(async (phrase: string) => {
        setConnecting(true);
        try {
            useLobbyStore.getState().reset();
            useMatchStore.getState().clearMatch();

            // importWallet returns the connection (safe, no secrets) + an ephemeral private key
            const { connection, ephemeralPrivateKeyHex } = await importWallet(phrase);

            try {
                const challenge = await apiClient.post<WalletChallengeResponse>('/auth/wallet-challenge', {
                    kaspa_address: connection.address,
                });

                // Sign using the ephemeral key — used once and then discarded
                const signature = await signMessage({
                    message: challenge.data.message,
                    privateKey: ephemeralPrivateKeyHex,
                    noAuxRand: true,
                });
                // ephemeralPrivateKeyHex goes out of scope after this block

                await apiClient.post('/auth/wallet-verify', {
                    challenge_id: challenge.data.challenge_id,
                    kaspa_address: connection.address,
                    public_key: connection.account.publicKey,
                    signature,
                    link_to_existing_user: isAuthenticated,
                });
            } catch (apiError) {
                console.error('[useWallet] Wallet authentication failed', apiError);
                throw new Error('Wallet authentication failed. Please try again.');
            }

            // Wallet auth succeeded — set connection state (NO secrets in store)
            setWalletConnection(connection.account, connection.address, 'mnemonic');
            updateKasAddress(connection.address);
            setWalletConnected(true);

            // Fetch user separately — don't fail wallet connect if this errors
            try {
                await fetchUser();
            } catch (fetchErr) {
                console.warn('[useWallet] fetchUser after wallet connect failed (non-fatal):', fetchErr);
            }

            await fetchBalance(connection.address);
            await subscribeToUpdates(connection.address);
        } catch (err: any) {
            setError(err.message || 'Connection failed');
        } finally {
            setConnecting(false);
        }
    }, [setWalletConnection, updateKasAddress, setWalletConnected, fetchBalance, subscribeToUpdates, setError, setConnecting, fetchUser, isAuthenticated]);

    const disconnect = useCallback(async () => {
        storeDisconnect();
        useLobbyStore.getState().reset();
        useMatchStore.getState().clearMatch();
        clearKasAddress();
        setWalletConnected(false);
        localStorage.removeItem(LEGACY_WALLET_SESSION_KEY);
        sessionStorage.removeItem(LEGACY_WALLET_PHRASE_KEY);
        subscriptionActive.current = false;
        try {
            await apiClient.post('/auth/me/wallet/disconnect');
            await fetchUser();
        } catch (error) {
            console.warn('[useWallet] Wallet disconnect sync failed', error);
        }
    }, [storeDisconnect, clearKasAddress, setWalletConnected, fetchUser]);

    useEffect(() => {
        restoreFullWalletState();
    }, [restoreFullWalletState]);

    useEffect(() => {
        if (isConnected && address) {
            fetchBalance(address);
        }
    }, [isConnected, address, fetchBalance]);

    const signAndSendDeposit = useCallback(async (matchId: string, amountKas: number) => {
        try {
            console.log(`[useWallet] Signing deposit for match ${matchId} with ${amountKas} KAS`);

            const txHash = `simulated_tx_${Math.random().toString(36).substring(7)}`;

            await apiClient.post('/matches/deposit', {
                match_id: matchId,
                tx_hash: txHash,
            });

            if (address) await fetchBalance(address);
        } catch (e: any) {
            console.error('[useWallet] Deposit failed:', e);
            throw e;
        }
    }, [address, fetchBalance]);

    return {
        address,
        isConnected,
        isConnecting,
        balanceSompi,
        isFetchingBalance,
        balanceError,
        error,
        connectWithMnemonic,
        disconnect,
        fetchBalance,
        restoreFullWalletState,
        signAndSendDeposit,
    };
}
