import { useCallback, useEffect, useRef } from 'react';
import { signMessage } from 'kaspa-wasm';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { useLobbyStore } from '../stores/useLobbyStore';
import { useMatchStore } from '../stores/useMatchStore';
import { importWallet, getBalanceByAddress, getRpcClient, sendKasFromWallet } from '../kaspa/wallet';
import apiClient from '../api/client';
import { submitTournamentDeposit } from '../api/tournaments';
import { getErrorMessage } from '../utils/errors';

const LEGACY_WALLET_SESSION_KEY = 'kaspa_wallet_session';
const LEGACY_WALLET_PHRASE_KEY = 'kaspa_encrypted_phrase';

interface WalletChallengeResponse {
    challenge_id: string;
    message: string;
    expires_at: string;
}

type RpcClientWithEvents = Awaited<ReturnType<typeof getRpcClient>> & {
    addEventListener?: (event: 'utxos-changed', listener: () => void) => void;
};

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

    const { updateKasAddress, clearKasAddress, setWalletConnected, fetchUser } = useAuthStore();
    const subscriptionActive = useRef(false);
    const fetchBalancePromise = useRef<Promise<void> | null>(null);

    const fetchBalance = useCallback(async (addr: string, force = false) => {
        if (!addr) return;
        
        if (fetchBalancePromise.current && !force) {
            return fetchBalancePromise.current;
        }

        const doFetch = async () => {
            setFetchingBalance(true);
            try {
                let sompi = await getBalanceByAddress(addr);
                
                // Kaspa nodes sometimes return 0 initially if utxos are not fully indexed for the connection yet
                let attempts = 0;
                while (sompi === 0 && attempts < 3) {
                    attempts++;
                    await new Promise(r => setTimeout(r, 1000));
                    sompi = await getBalanceByAddress(addr);
                }

                setBalance(sompi);
            } catch (e) {
                setBalanceError(getErrorMessage(e, '--'));
            } finally {
                fetchBalancePromise.current = null;
            }
        };

        fetchBalancePromise.current = doFetch();
        await fetchBalancePromise.current;
    }, [setBalance, setBalanceError, setFetchingBalance]);

    const subscribeToUpdates = useCallback(async (addr: string) => {
        if (subscriptionActive.current) return;
        try {
            const rpc = await getRpcClient();
            await rpc.subscribeUtxosChanged([addr]);

            const handleUtxoChanged = () => {
                console.log('[useWallet] UTXO changed, re-fetching balance...');
                void fetchBalance(addr);
            };

            (rpc as RpcClientWithEvents).addEventListener?.('utxos-changed', handleUtxoChanged);
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

            const { connection, ephemeralPrivateKeyHex } = await importWallet(phrase);

            try {
                let challenge;
                try {
                    challenge = await apiClient.post<WalletChallengeResponse>('/auth/wallet-challenge', {
                        kaspa_address: connection.address,
                    });
                } catch (challengeError: unknown) {
                    const status = (challengeError as { response?: { status?: number } })?.response?.status;
                    if (status === 429) {
                        throw new Error('Zu viele Versuche. Bitte warte eine Minute und versuche es erneut.');
                    }
                    // 403/401 = stale session token in localStorage but no valid cookie on backend (e.g. after restart)
                    // Clear the phantom auth state and retry as unauthenticated user
                    if (status === 403 || status === 401) {
                        console.warn('[useWallet] Stale session detected (403/401 on wallet-challenge), clearing auth state and retrying...');
                        useAuthStore.getState().clearAuthState();
                        try {
                            challenge = await apiClient.post<WalletChallengeResponse>('/auth/wallet-challenge', {
                                kaspa_address: connection.address,
                            });
                        } catch (retryError: unknown) {
                            const retryStatus = (retryError as { response?: { status?: number } })?.response?.status;
                            if (retryStatus === 429) {
                                throw new Error('Zu viele Versuche. Bitte warte eine Minute und versuche es erneut.');
                            }
                            throw retryError;
                        }
                    } else {
                        throw challengeError;
                    }
                }

                const signature = await signMessage({
                    message: challenge.data.message,
                    privateKey: ephemeralPrivateKeyHex,
                    noAuxRand: true,
                });

                // isAuthenticated may have changed after clearing stale state above
                const currentlyAuthenticated = useAuthStore.getState().isAuthenticated;
                await apiClient.post('/auth/wallet-verify', {
                    challenge_id: challenge.data.challenge_id,
                    kaspa_address: connection.address,
                    public_key: connection.account.publicKey,
                    signature,
                    link_to_existing_user: currentlyAuthenticated,
                });
            } catch (apiError) {
                console.error('[useWallet] Wallet authentication failed', apiError);
                throw apiError instanceof Error ? apiError : new Error('Wallet authentication failed. Please try again.');
            }

            setWalletConnection(connection.account, connection.address, 'mnemonic');
            updateKasAddress(connection.address);
            setWalletConnected(true);

            try {
                await fetchUser();
            } catch (fetchErr) {
                console.warn('[useWallet] fetchUser after wallet connect failed (non-fatal):', fetchErr);
            }

            await subscribeToUpdates(connection.address);
            await new Promise(r => setTimeout(r, 500)); // Wait for subscription to register
            await fetchBalance(connection.address, true);
        } catch (err) {
            setError(getErrorMessage(err, 'Connection failed'));
        } finally {
            setConnecting(false);
        }
    }, [fetchBalance, fetchUser, setConnecting, setError, setWalletConnected, setWalletConnection, subscribeToUpdates, updateKasAddress]);

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
        } catch (syncError) {
            console.warn('[useWallet] Wallet disconnect sync failed', syncError);
        }
    }, [clearKasAddress, fetchUser, setWalletConnected, storeDisconnect]);

    useEffect(() => {
        void restoreFullWalletState();
    }, [restoreFullWalletState]);

    useEffect(() => {
        if (isConnected && address) {
            void fetchBalance(address);
        }
    }, [address, fetchBalance, isConnected]);

    const signAndSendDeposit = useCallback(async (matchId: string, amountKas: number, escrowAddress: string) => {
        if (!address) {
            throw new Error('Bitte verbinde zuerst deine Kaspa Wallet.');
        }

        try {
            console.log(`[useWallet] Sending REAL deposit for match ${matchId} with ${amountKas} KAS`);

            const amountSompi = BigInt(Math.round(amountKas * 100_000_000));
            const { txId } = await sendKasFromWallet(address, escrowAddress, amountSompi);

            await apiClient.post('/matches/deposit', {
                match_id: matchId,
                tx_hash: txId,
            });

            await new Promise(r => setTimeout(r, 1500));
            await fetchBalance(address, true);
            
            return txId;
        } catch (e) {
            console.error('[useWallet] Deposit failed:', e);
            throw e;
        }
    }, [address, fetchBalance]);

    const signAndSendTournamentDeposit = useCallback(
        async (tournamentId: string, teamId: string, amountKas: number, escrowAddress: string) => {
            if (!address) {
                throw new Error('Bitte verbinde zuerst deine Kaspa Wallet.');
            }

            try {
                console.log(`[useWallet] Sending REAL deposit for tournament ${tournamentId}, team ${teamId} with ${amountKas} KAS`);
                
                const amountSompi = BigInt(Math.round(amountKas * 100_000_000));
                const { txId } = await sendKasFromWallet(address, escrowAddress, amountSompi);

                await submitTournamentDeposit(tournamentId, teamId, txId);

                await new Promise(r => setTimeout(r, 1500));
                await fetchBalance(address, true);

                return txId;
            } catch (e) {
                console.error('[useWallet] Tournament deposit failed:', e);
                throw e;
            }
        },
        [address, fetchBalance]
    );

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
        signAndSendTournamentDeposit,
    };
}
