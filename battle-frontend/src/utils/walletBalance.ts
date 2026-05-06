export const hasValidBalance = (balanceSompi: number) => balanceSompi >= 0;

export const getBalanceKasSafe = (balanceSompi: number) =>
  hasValidBalance(balanceSompi) ? balanceSompi / 100_000_000 : 0;
