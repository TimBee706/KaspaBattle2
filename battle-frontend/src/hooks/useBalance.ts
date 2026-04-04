import { useEffect } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { getBalance } from '../kaspa/wallet';

// Pollt die Balance alle 30 Sekunden als Fallback zum Event-Listener
export function useBalance() {
    const { account, address, setBalance, isConnected } = useWalletStore();

    useEffect(() => {
        if (!account || !address || !isConnected) return;

        const fetchBalance = async () => {
            try {
                // Fetch using the account xpub directly
                const balance = await getBalance(account.xpub);
                setBalance(balance);
            } catch {
                // Silent fail – Event-Listener ist primär
            }
        };

        fetchBalance();
        const interval = setInterval(fetchBalance, 30_000);

        return () => clearInterval(interval);
    }, [address, isConnected, setBalance]);
}
