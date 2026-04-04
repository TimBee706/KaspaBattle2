import { useEffect } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { getBalance } from '../kaspa/wallet';

export function useBalance() {
    const { account, address, setBalance, isConnected } = useWalletStore();

    useEffect(() => {
        if (!account || !address || !isConnected) return;

        const fetchBalance = async () => {
            try {
                const balance = await getBalance(account.xpub);
                setBalance(balance);
            } catch {
                // Silent fail - event listener is primary
            }
        };

        void fetchBalance();
        const interval = setInterval(fetchBalance, 30_000);

        return () => clearInterval(interval);
    }, [account, address, isConnected, setBalance]);
}
