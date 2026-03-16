/**
 * KaspaBattle — TX Review & Signing Helpers
 *
 * In the backend-held keys model, the frontend does NOT sign transactions.
 * This module provides:
 * 1. TX review/display helpers for the deposit flow
 * 2. Types for future migration to client-side signing (SilverScript)
 * 3. Time-lock display utilities
 */

import { sompiToKas, explorerTxUrl, explorerAddressUrl } from './multisig';

// ─── TX Review Types ─────────────────────────────────────────────────────────

/** A simplified view of a transaction for user review */
export interface TxReviewInfo {
  /** Human-readable description */
  description: string;
  /** Inputs being spent */
  inputs: TxReviewInput[];
  /** Outputs being created */
  outputs: TxReviewOutput[];
  /** Network fee in sompi */
  feeSompi: number;
  /** Fee in KAS */
  feeKas: string;
  /** Total amount being moved */
  totalSompi: number;
}

export interface TxReviewInput {
  address: string;
  amountSompi: number;
  amountKas: string;
  /** Is this from the escrow P2SH address? */
  isEscrow: boolean;
}

export interface TxReviewOutput {
  address: string;
  amountSompi: number;
  amountKas: string;
  /** Role of this output */
  role: 'winner' | 'refund_a' | 'refund_b' | 'treasury' | 'change' | 'unknown';
}

// ─── Time-Lock Display Utilities ─────────────────────────────────────────────

/** Information about a time-locked escrow's timeout */
export interface TimeLockInfo {
  /** Unix timestamp when the time-lock expires */
  expiresAt: number;
  /** Whether the time-lock has expired */
  isExpired: boolean;
  /** Human-readable time remaining */
  timeRemaining: string;
  /** Human-readable expiry date */
  expiryDate: string;
}

/**
 * Calculate time-lock display info from a Unix timestamp.
 */
export function getTimeLockInfo(timelockTimestamp: number): TimeLockInfo {
  const now = Math.floor(Date.now() / 1000);
  const isExpired = now >= timelockTimestamp;
  const remainingSeconds = Math.max(0, timelockTimestamp - now);

  return {
    expiresAt: timelockTimestamp,
    isExpired,
    timeRemaining: formatDuration(remainingSeconds),
    expiryDate: new Date(timelockTimestamp * 1000).toLocaleString(),
  };
}

/**
 * Format a duration in seconds into a human-readable string.
 * Examples: "2h 30m", "45m", "15s", "Abgelaufen"
 */
export function formatDuration(seconds: number): string {
  if (seconds <= 0) return 'Abgelaufen';

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

// ─── TX Review Builder ───────────────────────────────────────────────────────

/**
 * Build a TxReviewInfo for a payout transaction.
 *
 * This transforms raw backend data into a user-friendly review format.
 */
export function buildPayoutReview(params: {
  escrowAddress: string;
  escrowBalanceSompi: number;
  winnerAddress: string;
  winnerAmountSompi: number;
  treasuryAddress: string;
  platformFeeSompi: number;
  networkFeeSompi: number;
}): TxReviewInfo {
  return {
    description: 'Auszahlung an den Gewinner',
    inputs: [
      {
        address: params.escrowAddress,
        amountSompi: params.escrowBalanceSompi,
        amountKas: sompiToKas(params.escrowBalanceSompi, 4),
        isEscrow: true,
      },
    ],
    outputs: [
      {
        address: params.winnerAddress,
        amountSompi: params.winnerAmountSompi,
        amountKas: sompiToKas(params.winnerAmountSompi, 4),
        role: 'winner',
      },
      {
        address: params.treasuryAddress,
        amountSompi: params.platformFeeSompi,
        amountKas: sompiToKas(params.platformFeeSompi, 4),
        role: 'treasury',
      },
    ],
    feeSompi: params.networkFeeSompi,
    feeKas: sompiToKas(params.networkFeeSompi, 4),
    totalSompi: params.escrowBalanceSompi,
  };
}

/**
 * Build a TxReviewInfo for a refund transaction.
 */
export function buildRefundReview(params: {
  escrowAddress: string;
  escrowBalanceSompi: number;
  playerAAddress: string;
  playerBAddress: string;
  networkFeeSompi: number;
}): TxReviewInfo {
  const netBalance = params.escrowBalanceSompi - params.networkFeeSompi;
  const halfAmount = Math.floor(netBalance / 2);

  return {
    description: 'Rückerstattung an beide Spieler',
    inputs: [
      {
        address: params.escrowAddress,
        amountSompi: params.escrowBalanceSompi,
        amountKas: sompiToKas(params.escrowBalanceSompi, 4),
        isEscrow: true,
      },
    ],
    outputs: [
      {
        address: params.playerAAddress,
        amountSompi: halfAmount,
        amountKas: sompiToKas(halfAmount, 4),
        role: 'refund_a',
      },
      {
        address: params.playerBAddress,
        amountSompi: netBalance - halfAmount,
        amountKas: sompiToKas(netBalance - halfAmount, 4),
        role: 'refund_b',
      },
    ],
    feeSompi: params.networkFeeSompi,
    feeKas: sompiToKas(params.networkFeeSompi, 4),
    totalSompi: params.escrowBalanceSompi,
  };
}

// ─── Explorer Link Helpers (re-exported for convenience) ─────────────────────

export { explorerTxUrl, explorerAddressUrl };
