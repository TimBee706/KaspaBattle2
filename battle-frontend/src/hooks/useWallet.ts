import { useCallback, useEffect, useRef } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { importWallet, getBalanceByAddress, getRpcClient } from '../kaspa/wallet';

const SESSION_KEY = 'kaspa_wallet_session';
const STORAGE_PHRASE_KEY = 'kaspa_encrypted_phrase';
const SESSION_TTL = 7 * 24 * 60 * 60 * 1000; // 7 days

export interface WalletSession {
    address: string;
    walletType: 'mnemonic' | 'kasware';
    connectedAt: number;
}

// Simple XOR encryption for sessionStorage (not production-grade but better than plaintext)
// In a real app, use Web Crypto API with high-entropy keys
const xorEncrypt = (text: string, key: string) => {
    return btoa(text.split('').map((char, i) =>
        String.fromCharCode(char.charCodeAt(0) ^ key.charCodeAt(i % key.length))
    ).join(''));
};

const xorDecrypt = (encoded: string, key: string) => {
    try {
        const text = atob(encoded);
        return text.split('').map((char, i) =>
            String.fromCharCode(char.charCodeAt(0) ^ key.charCodeAt(i % key.length))
        ).join('');
    } catch (e) {
        return null;
    }
};

const CRYPTO_KEY = "kaspabattle_internal_key"; // In production, this should be derived from a user password

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
        disconnect: storeDisconnect
    } = useWalletStore();

    const { updateKasAddress, setWalletConnected } = useAuthStore();
    const isReconnecting = useRef(false);
    const subscriptionActive = useRef(false);

    const fetchBalance = useCallback(async (addr: string) => {
        setFetchingBalance(true);
        try {
            const bal = await getBalanceByAddress(addr);
            setBalance(bal);
        } catch (e: any) {
            setBalanceError(e.message || "Balance error");
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

    const connectWithMnemonic = useCallback(async (phrase: string) => {
        setConnecting(true);
        try {
            const connection = await importWallet(phrase);

            // Update Store
            setWalletConnection(
                connection.wallet,
                connection.account,
                connection.address,
                connection.mnemonic,
                'mnemonic'
            );

            // Update Auth Store
            updateKasAddress(connection.address);
            setWalletConnected(true);

            // Persist Session (localStorage)
            const session: WalletSession = {
                address: connection.address,
                walletType: 'mnemonic',
                connectedAt: Date.now(),
            };
            localStorage.setItem(SESSION_KEY, JSON.stringify(session));

            // Persist Mnemonic (sessionStorage - encrypted)
            const encrypted = xorEncrypt(phrase, CRYPTO_KEY);
            sessionStorage.setItem(STORAGE_PHRASE_KEY, encrypted);

            // Initial Balance & Subscription
            await fetchBalance(connection.address);
            await subscribeToUpdates(connection.address);

        } catch (err: any) {
            setError(err.message || "Connection failed");
        }
    }, [setWalletConnection, updateKasAddress, setWalletConnected, fetchBalance, subscribeToUpdates, setError, setConnecting]);

    const disconnect = useCallback(() => {
        storeDisconnect();
        setWalletConnected(false);
        localStorage.removeItem(SESSION_KEY);
        sessionStorage.removeItem(STORAGE_PHRASE_KEY);
        subscriptionActive.current = false;
    }, [storeDisconnect, setWalletConnected]);

    // AUTO-RECONNECT & INITIAL FETCH
    useEffect(() => {
        const restore = async () => {
            if (isConnected || isReconnecting.current) return;

            const sessionRaw = localStorage.getItem(SESSION_KEY);
            if (!sessionRaw) return;

            try {
                const session: WalletSession = JSON.parse(sessionRaw);

                // Check TTL
                if (Date.now() - session.connectedAt > SESSION_TTL) {
                    disconnect();
                    return;
                }

                isReconnecting.current = true;

                // Sync Auth Store
                setWalletConnected(true);
                updateKasAddress(session.address);

                // Restore Mnemonic from sessionStorage if available
                const encrypted = sessionStorage.getItem(STORAGE_PHRASE_KEY);
                let restoredMnemonic = null;
                if (encrypted) {
                    restoredMnemonic = xorDecrypt(encrypted, CRYPTO_KEY);
                }

                // Restore Store State
                useWalletStore.setState({
                    isConnected: true,
                    address: session.address,
                    walletType: session.walletType,
                    mnemonic: restoredMnemonic
                });

                // Fetch Balance immediately
                await fetchBalance(session.address);
                await subscribeToUpdates(session.address);

            } catch (e) {
                console.error('[useWallet] Restore failed:', e);
                disconnect();
            } finally {
                isReconnecting.current = false;
            }
        };

        restore();
    }, [isConnected, setWalletConnected, updateKasAddress, fetchBalance, subscribeToUpdates, disconnect]);

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
        fetchBalance
    };
}
