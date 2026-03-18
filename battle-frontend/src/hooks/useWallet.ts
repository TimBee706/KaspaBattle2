import { useCallback, useEffect, useRef } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { importWallet, getBalanceByAddress, getRpcClient } from '../kaspa/wallet';
import apiClient from '../api/client';

const KEY_SESSION = 'kaspa_wallet_session';
const KEY_PHRASE = 'kaspa_encrypted_phrase';
const SESSION_TTL = 7 * 24 * 60 * 60 * 1000;

const INTERNAL_CRYPTO_KEY = 'kaspabattle_v1_secure_key';

async function encryptMnemonic(text: string, password = INTERNAL_CRYPTO_KEY): Promise<string> {
    const encoder = new TextEncoder();
    const data = encoder.encode(text);

    const passwordKey = await crypto.subtle.importKey(
        'raw',
        encoder.encode(password),
        { name: 'PBKDF2' },
        false,
        ['deriveKey'],
    );

    const salt = crypto.getRandomValues(new Uint8Array(16));
    const aesKey = await crypto.subtle.deriveKey(
        { name: 'PBKDF2', salt, iterations: 100000, hash: 'SHA-256' },
        passwordKey,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt'],
    );

    const iv = crypto.getRandomValues(new Uint8Array(12));
    const encrypted = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, aesKey, data);

    const combined = new Uint8Array(salt.length + iv.length + encrypted.byteLength);
    combined.set(salt, 0);
    combined.set(iv, salt.length);
    combined.set(new Uint8Array(encrypted), salt.length + iv.length);

    return btoa(String.fromCharCode(...combined));
}

async function decryptMnemonic(base64: string, password = INTERNAL_CRYPTO_KEY): Promise<string | null> {
    try {
        const combined = new Uint8Array(atob(base64).split('').map((c) => c.charCodeAt(0)));
        const salt = combined.slice(0, 16);
        const iv = combined.slice(16, 28);
        const data = combined.slice(28);

        const encoder = new TextEncoder();
        const passwordKey = await crypto.subtle.importKey(
            'raw',
            encoder.encode(password),
            { name: 'PBKDF2' },
            false,
            ['deriveKey'],
        );

        const aesKey = await crypto.subtle.deriveKey(
            { name: 'PBKDF2', salt, iterations: 100000, hash: 'SHA-256' },
            passwordKey,
            { name: 'AES-GCM', length: 256 },
            false,
            ['decrypt'],
        );

        const decrypted = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, aesKey, data);
        return new TextDecoder().decode(decrypted);
    } catch (e) {
        console.error('[Crypto] Decryption failed:', e);
        return null;
    }
}

export function useWallet() {
    const {
        address,
        isConnected,
        isConnecting,
        balanceSompi,
        isFetchingBalance,
        balanceError,
        mnemonic,
        error,
        setWalletConnection,
        setBalance,
        setFetchingBalance,
        setBalanceError,
        setConnecting,
        setError,
        disconnect: storeDisconnect,
    } = useWalletStore();

    const { updateKasAddress, setWalletConnected } = useAuthStore();
    const isReconnecting = useRef(false);
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
        if (isReconnecting.current) return;

        const sessionRaw = localStorage.getItem(KEY_SESSION);
        if (!sessionRaw) return;

        try {
            const session: any = JSON.parse(sessionRaw);

            if (Date.now() - session.connectedAt > SESSION_TTL) {
                localStorage.removeItem(KEY_SESSION);
                return;
            }

            isReconnecting.current = true;

            setWalletConnected(true);
            updateKasAddress(session.address);

            const encryptedToken = sessionStorage.getItem(KEY_PHRASE);
            let restoredMnemonic = null;
            if (encryptedToken) {
                restoredMnemonic = await decryptMnemonic(encryptedToken);
            }

            useWalletStore.setState({
                isConnected: true,
                address: session.address,
                walletType: session.walletType,
                mnemonic: restoredMnemonic,
            });

            await fetchBalance(session.address);
            await subscribeToUpdates(session.address);
        } catch (e) {
            console.error('[useWallet] Restore sequence failed:', e);
        } finally {
            isReconnecting.current = false;
        }
    }, [setWalletConnected, updateKasAddress, fetchBalance, subscribeToUpdates]);

    const connectWithMnemonic = useCallback(async (phrase: string) => {
        setConnecting(true);
        try {
            const connection = await importWallet(phrase);

            const message = `KaspaBattle Login-Request: ${Date.now()}`;
            const signature = `simulated_signature_${Math.random().toString(36).substring(7)}`;
            const { isAuthenticated, tokens, setTokens, fetchUser } = useAuthStore.getState();

            try {
                const response = await apiClient.post('/auth/wallet-login', {
                    kaspa_address: connection.address,
                    message,
                    signature,
                });

                const { session_token } = response.data;
                const existingToken = tokens?.access_token;

                if (session_token && session_token !== existingToken) {
                    setTokens({
                        access_token: session_token,
                        refresh_token: tokens?.refresh_token ?? '',
                        expires_at: Math.floor(Date.now() / 1000) + 7 * 24 * 60 * 60,
                    });
                }

                await fetchUser().catch(console.error);
            } catch (apiError) {
                console.error('[useWallet] Backend auth failed - check if backend is running', apiError);
                if (!isAuthenticated) {
                    console.warn('[useWallet] Continuing with local-only wallet session');
                }
            }

            setWalletConnection(null, connection.account, connection.address, phrase, 'mnemonic');
            updateKasAddress(connection.address);
            setWalletConnected(true);

            const session: any = {
                address: connection.address,
                walletType: 'mnemonic',
                connectedAt: Date.now(),
            };
            localStorage.setItem(KEY_SESSION, JSON.stringify(session));

            const encrypted = await encryptMnemonic(phrase);
            sessionStorage.setItem(KEY_PHRASE, encrypted);

            await fetchBalance(connection.address);
            await subscribeToUpdates(connection.address);
        } catch (err: any) {
            setError(err.message || 'Connection failed');
        }
    }, [setWalletConnection, updateKasAddress, setWalletConnected, fetchBalance, subscribeToUpdates, setError, setConnecting]);

    const disconnect = useCallback(() => {
        storeDisconnect();
        setWalletConnected(false);
        localStorage.removeItem(KEY_SESSION);
        sessionStorage.removeItem(KEY_PHRASE);
        subscriptionActive.current = false;
    }, [storeDisconnect, setWalletConnected]);

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
        mnemonic,
        error,
        connectWithMnemonic,
        disconnect,
        fetchBalance,
        restoreFullWalletState,
        signAndSendDeposit,
    };
}
