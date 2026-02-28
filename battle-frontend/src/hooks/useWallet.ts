export const useWallet = () => {
    const signAndSendDeposit = async (_matchId: string, _amount: number) => {
        // Implementation logic for kaspa-wasm signature integration
        return new Promise((resolve) => setTimeout(() => resolve("tx_hash_mock"), 1500));
    };
    return { signAndSendDeposit };
};
