/**
 * useMultisigEscrow — React hook for multisig escrow interaction.
 *
 * Wraps the multisig API to provide:
 * - Escrow creation for new matches
 * - Deposit status polling (10s interval)
 * - Status-aware display helpers
 *
 * Usage:
 * ```tsx
 * const { escrowInfo, depositStatus, createEscrow, pollDeposits } = useMultisigEscrow(matchId);
 * ```
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import {
  type MultisigEscrowInfo,
  type MultisigDepositStatus,
  createMultisigEscrow,
  checkDepositStatus,
  getEscrowDetails,
} from '../kaspa/multisig';

/** Polling interval for deposit status: 10 seconds */
const DEPOSIT_POLL_INTERVAL_MS = 10_000;

export function useMultisigEscrow(matchId: string | null) {
  const [escrowInfo, setEscrowInfo] = useState<MultisigEscrowInfo | null>(null);
  const [depositStatus, setDepositStatus] = useState<MultisigDepositStatus | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Fetch existing escrow details on mount
  useEffect(() => {
    if (!matchId) return;

    getEscrowDetails(matchId)
      .then((info) => {
        if (info) setEscrowInfo(info);
      })
      .catch((err) => {
        console.warn('[useMultisigEscrow] Could not fetch existing escrow:', err);
      });
  }, [matchId]);

  // Create a new multisig escrow
  const createEscrow = useCallback(
    async (wagerPerPlayerSompi: number, timelockTimestamp?: number) => {
      if (!matchId) {
        setError('No match ID');
        return;
      }

      setIsCreating(true);
      setError(null);

      try {
        const info = await createMultisigEscrow(matchId, wagerPerPlayerSompi, timelockTimestamp);
        setEscrowInfo(info);
        console.log('✅ [useMultisigEscrow] Escrow created:', info.escrowAddress);
        return info;
      } catch (err: any) {
        const msg = err.message || 'Failed to create multisig escrow';
        setError(msg);
        console.error('❌ [useMultisigEscrow] Create failed:', msg);
      } finally {
        setIsCreating(false);
      }
    },
    [matchId]
  );

  // Poll deposit status
  const pollDeposits = useCallback(async () => {
    if (!escrowInfo) return;

    try {
      const status = await checkDepositStatus(
        escrowInfo.escrowAddress,
        escrowInfo.wagerPerPlayerSompi
      );
      setDepositStatus(status);
      return status;
    } catch (err: any) {
      console.warn('[useMultisigEscrow] Deposit check failed:', err.message);
    }
  }, [escrowInfo]);

  // Auto-poll deposits when escrow exists and is not fully funded
  useEffect(() => {
    if (!escrowInfo) return;
    if (depositStatus?.isFullyFunded) {
      // Stop polling once fully funded
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
      return;
    }

    setIsPolling(true);
    // Initial poll
    pollDeposits();

    // Interval polling
    pollRef.current = setInterval(pollDeposits, DEPOSIT_POLL_INTERVAL_MS);

    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
      setIsPolling(false);
    };
  }, [escrowInfo, depositStatus?.isFullyFunded, pollDeposits]);

  return {
    escrowInfo,
    depositStatus,
    isCreating,
    isPolling,
    error,
    createEscrow,
    pollDeposits,
  };
}
