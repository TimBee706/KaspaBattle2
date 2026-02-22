import { useEffect } from 'react';
import { useWalletStore } from '../stores/useWalletStore';
import { getBalance } from '../kaspa/wallet';

// Pollt die Balance alle 30 Sekunden als Fallback zum Event-Listener
export function useBalance() {
    const { address, setBalance, isConnected } = useWalletStore();

    useEffect(() => {
        if (!address || !isConnected) return;

        const fetchBalance = async () => {
            try {
                const balance = await getBalance(address);
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
