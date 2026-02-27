import { useCallback } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { importWallet, onBalanceChange, getBalance } from '../kaspa/wallet';

export function useWalletConnect() {
    const {
        isConnected, isConnecting, address, mnemonic, balanceSompi, error,
        setWalletConnection, setBalance, setConnecting, setError, disconnect,
    } = useWalletStore();
    const { updateKasAddress } = useAuthStore();


    const connectWithMnemonic = useCallback(async (mnemonic: string) => {
        setConnecting(true);
        try {
            const connection = await importWallet(mnemonic);
            setWalletConnection(connection.wallet, connection.account, connection.address, connection.mnemonic);
            updateKasAddress(connection.address);

            // Fetch balance using the account directly from the WASM binding
            const initialBalance = await getBalance(connection.account);
            setBalance(initialBalance);

            // Register event listener with both wallet and account context
            onBalanceChange(connection.wallet, connection.account, setBalance);
        } catch (err: any) {
            setError(err.message);
        }
    }, [setWalletConnection, setBalance, setConnecting, setError, updateKasAddress]);

    return {
        isConnected, isConnecting, address, mnemonic, balanceSompi, error,
        connectWithMnemonic, disconnect,
    };
}
