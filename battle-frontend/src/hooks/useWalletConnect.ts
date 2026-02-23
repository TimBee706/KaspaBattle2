import { useCallback } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { useAuthStore } from '../stores/useAuthStore';
import { importWallet, onBalanceChange, getBalance } from '../kaspa/wallet';

export function useWalletConnect() {
    const {
        isConnected, isConnecting, address, balanceSompi, error,
        setWalletConnection, setBalance, setConnecting, setError, disconnect,
    } = useWalletStore();
    const { updateKasAddress } = useAuthStore();


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
        connectWithMnemonic, disconnect,
    };
}
