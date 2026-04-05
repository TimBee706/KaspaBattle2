/**
 * KaspaBattle Multisig Escrow — TypeScript Types & Helpers
 *
 * Provides interfaces and utility functions for the frontend
 * to interact with the backend multisig escrow system.
 *
 * The frontend does NOT handle signing (backend-held keys model).
 * It only needs to:
 * 1. Request escrow creation from the backend
 * 2. Display the P2SH address for deposits
 * 3. Track deposit status
 * 4. Display payout/refund results
 */

// ─── Escrow Interfaces ───────────────────────────────────────────────────────

/** Status of a multisig escrow through its lifecycle */
export type EscrowStatus =
  | 'Created'
  | 'PartiallyFunded'
  | 'Funded'
  | 'Settling'
  | 'Settled'
  | 'Refunding'
  | 'Refunded'
  | 'TimedOut'
  | 'Disputed';

/** Information about a multisig escrow (returned by backend on creation) */
export interface MultisigEscrowInfo {
  matchId: string;
  escrowAddress: string;
  redeemScriptHex: string;
  pubkeys: [string, string, string]; // [playerA, playerB, oracle]
  threshold: number;
  wagerPerPlayerSompi: number;
  wagerPerPlayerKas: number;
  timelockTimestamp?: number;
}

/** Deposit status for a multisig escrow */
export interface MultisigDepositStatus {
  depositedSompi: number;
  requiredSompi: number;
  isFullyFunded: boolean;
  utxoCount: number;
}

/** Result of a successful multisig payout */
export interface MultisigPayoutResult {
  matchId: string;
  txId: string;
  winnerAddress: string;
  winnerAmountSompi: number;
  platformFeeSompi: number;
  networkFeeSompi: number;
  timestamp: string;
}

// ─── API Helpers ─────────────────────────────────────────────────────────────

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

/**
 * Request the backend to create a new multisig escrow for a match.
 *
 * @param matchId - UUID of the match
 * @param wagerPerPlayerSompi - Wager amount per player in sompi
 * @param timelockTimestamp - Optional time-lock (Unix seconds)
 */
export async function createMultisigEscrow(
  matchId: string,
  wagerPerPlayerSompi: number,
  timelockTimestamp?: number
): Promise<MultisigEscrowInfo> {
  const response = await fetch(`${API_BASE}/api/v1/multisig/create`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      matchId,
      wagerPerPlayerSompi,
      timelockTimestamp,
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Failed to create escrow: ${error}`);
  }

  return response.json();
}

/**
 * Poll deposit status for an escrow address.
 *
 * @param escrowAddress - The P2SH address to check
 * @param wagerPerPlayerSompi - Expected wager per player
 */
export async function checkDepositStatus(
  escrowAddress: string,
  wagerPerPlayerSompi: number
): Promise<MultisigDepositStatus> {
  const response = await fetch(
    `${API_BASE}/api/v1/multisig/deposits?address=${encodeURIComponent(escrowAddress)}&wager=${wagerPerPlayerSompi}`
  );

  if (!response.ok) {
    throw new Error(`Failed to check deposits: ${await response.text()}`);
  }

  return response.json();
}

/**
 * Get the full details of a multisig escrow.
 *
 * @param matchId - UUID of the match
 */
export async function getEscrowDetails(
  matchId: string
): Promise<MultisigEscrowInfo | null> {
  const response = await fetch(`${API_BASE}/api/v1/multisig/${matchId}`);

  if (response.status === 404) return null;
  if (!response.ok) {
    throw new Error(`Failed to get escrow: ${await response.text()}`);
  }

  return response.json();
}

// ─── Formatting Utilities ────────────────────────────────────────────────────

const SOMPI_PER_KAS = 100_000_000;

/** Convert sompi to KAS with specified decimal places */
export function sompiToKas(sompi: number, decimals: number = 8): string {
  return (sompi / SOMPI_PER_KAS).toFixed(decimals);
}

/** Convert KAS to sompi */
export function kasToSompi(kas: number): number {
  return Math.round(kas * SOMPI_PER_KAS);
}

/** Shorten a Kaspa address for display: kaspatest:qr12...ab34 */
export function shortenAddress(address: string, chars: number = 6): string {
  if (address.length <= chars * 2 + 3) return address;
  const prefix = address.substring(0, address.indexOf(':') + 1 + chars);
  const suffix = address.substring(address.length - chars);
  return `${prefix}...${suffix}`;
}

/** Build a Kaspa explorer URL for a transaction */
export function explorerTxUrl(txId: string, network: 'mainnet' | 'testnet-10' | 'testnet-11' | 'testnet-12' = 'testnet-12'): string {
  const explorerBase = network === 'mainnet' ? 'https://kas.fyi' : 'https://testnet.kas.fyi';
  const baseUrl = `${explorerBase}/tx/${txId}`;
  return network.startsWith('testnet') ? `${baseUrl}?network=${network}` : baseUrl;
}

/** Build a Kaspa explorer URL for an address */
export function explorerAddressUrl(address: string, network: 'mainnet' | 'testnet-10' | 'testnet-11' | 'testnet-12' = 'testnet-12'): string {
  const explorerBase = network === 'mainnet' ? 'https://kas.fyi' : 'https://testnet.kas.fyi';
  const baseUrl = `${explorerBase}/address/${address}`;
  return network.startsWith('testnet') ? `${baseUrl}?network=${network}` : baseUrl;
}
