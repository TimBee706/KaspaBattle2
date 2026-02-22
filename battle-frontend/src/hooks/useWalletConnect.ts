import { useCallback } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { createNewWallet, importWallet, onBalanceChange, getBalance } from '../kaspa/wallet';

export function useWalletConnect() {
    const {
        isConnected, isConnecting, address, balanceSompi, error,
        setWalletConnection, setBalance, setConnecting, setError, disconnect,
    } = useWalletStore();
    const { updateKasAddress } = useAuthStore();

    const connectNewWallet = useCallback(async () => {
        setConnecting(true);
        try {
            const { connection, mnemonic } = await createNewWallet();
            setWalletConnection(connection.wallet, connection.account, connection.address);
            updateKasAddress(connection.address);

            // Balance-Tracking starten
            const initialBalance = await getBalance(connection.address);
            setBalance(initialBalance);
            onBalanceChange(connection.wallet, setBalance);

            return mnemonic; // UI muss Mnemonic dem User zeigen!
        } catch (err: any) {
            setError(err.message);
            return null;
        }
    }, [setWalletConnection, setBalance, setConnecting, setError, updateKasAddress]);

    const connectWithMnemonic = useCallback(async (mnemonic: string) => {
        setConnecting(true);
        try {
            const connection = await importWallet(mnemonic);
            setWalletConnection(connection.wallet, connection.account, connection.address);
            updateKasAddress(connection.address);

            const initialBalance = await getBalance(connection.address);
            setBalance(initialBalance);
            onBalanceChange(connection.wallet, setBalance);
        } catch (err: any) {
            setError(err.message);
        }
    }, [setWalletConnection, setBalance, setConnecting, setError, updateKasAddress]);

    return {
        isConnected, isConnecting, address, balanceSompi, error,
        connectNewWallet, connectWithMnemonic, disconnect,
    };
}
