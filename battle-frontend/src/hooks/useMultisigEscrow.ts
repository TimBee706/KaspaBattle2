import { useState, useEffect, useCallback, useRef } from 'react';
import {
    type MultisigEscrowInfo,
    type MultisigDepositStatus,
    createMultisigEscrow,
    checkDepositStatus,
    getEscrowDetails,
} from '../kaspa/multisig';
import { getErrorMessage } from '../utils/errors';

const DEPOSIT_POLL_INTERVAL_MS = 10_000;

export function useMultisigEscrow(matchId: string | null) {
    const [escrowInfo, setEscrowInfo] = useState<MultisigEscrowInfo | null>(null);
    const [depositStatus, setDepositStatus] = useState<MultisigDepositStatus | null>(null);
    const [isCreating, setIsCreating] = useState(false);
    const [isPolling, setIsPolling] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

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

    const createEscrow = useCallback(async (wagerPerPlayerSompi: number, timelockTimestamp?: number) => {
        if (!matchId) {
            setError('No match ID');
            return;
        }

        setIsCreating(true);
        setError(null);

        try {
            const info = await createMultisigEscrow(matchId, wagerPerPlayerSompi, timelockTimestamp);
            setEscrowInfo(info);
            console.log('[useMultisigEscrow] Escrow created:', info.escrowAddress);
            return info;
        } catch (err) {
            const msg = getErrorMessage(err, 'Failed to create multisig escrow');
            setError(msg);
            console.error('[useMultisigEscrow] Create failed:', msg);
        } finally {
            setIsCreating(false);
        }
    }, [matchId]);

    const pollDeposits = useCallback(async () => {
        if (!escrowInfo) return;

        try {
            const status = await checkDepositStatus(
                escrowInfo.escrowAddress,
                escrowInfo.wagerPerPlayerSompi,
            );
            setDepositStatus(status);
            return status;
        } catch (err) {
            console.warn('[useMultisigEscrow] Deposit check failed:', getErrorMessage(err));
        }
    }, [escrowInfo]);

    useEffect(() => {
        if (!escrowInfo) return;
        if (depositStatus?.isFullyFunded) {
            if (pollRef.current) {
                clearInterval(pollRef.current);
                pollRef.current = null;
            }
            return;
        }

        setIsPolling(true);
        void pollDeposits();
        pollRef.current = setInterval(pollDeposits, DEPOSIT_POLL_INTERVAL_MS);

        return () => {
            if (pollRef.current) {
                clearInterval(pollRef.current);
                pollRef.current = null;
            }
            setIsPolling(false);
        };
    }, [depositStatus?.isFullyFunded, escrowInfo, pollDeposits]);

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
