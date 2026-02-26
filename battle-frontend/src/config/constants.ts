// Kaspa
export const KASPA_NODE_URL = import.meta.env.VITE_KASPA_NODE_URL || 'wss://mainnet.kaspa.aspectron.org';
export const KASPA_NETWORK = import.meta.env.VITE_KASPA_NETWORK || 'mainnet';
export const KASPA_EXPLORER_URL = 'https://explorer.kaspa.org';
export const SOMPI_PER_KAS = 100_000;

// KaspaBattle API
export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:8080/api/v1';
export const WS_BASE_URL = import.meta.env.VITE_WS_BASE_URL || 'ws://localhost:8080/ws';

// FACEIT OAuth
export const FACEIT_CLIENT_ID = import.meta.env.VITE_FACEIT_CLIENT_ID || '139b1b6c-f499-4639-8272-4e07fc976588';
export const FACEIT_REDIRECT_URI = import.meta.env.VITE_FACEIT_REDIRECT_URI || 'https://nonminimal-secernent-ernestina.ngrok-free.dev/api/v1/faceit/callback';
export const FACEIT_AUTH_URL = 'https://accounts.faceit.com';
export const FACEIT_SCOPES = 'openid profile email';

// Match-Limits (aus Whitepaper)
export const MIN_WAGER_KAS = 10;
export const MAX_WAGER_KAS = 10_000;
export const MIN_WAGER_SOMPI = MIN_WAGER_KAS * SOMPI_PER_KAS;
export const MAX_WAGER_SOMPI = MAX_WAGER_KAS * SOMPI_PER_KAS;

// Fee-Struktur (aus Whitepaper)
export const FEE_WINNER_PERCENT = 95;
export const FEE_TREASURY_PERCENT = 3;
export const FEE_ORACLE_PERCENT = 2;

// Polling
export const MATCH_STATUS_POLL_INTERVAL_MS = 5_000;
export const LOBBY_POLL_INTERVAL_MS = 10_000;

// Unterstützte Spiele
export const SUPPORTED_GAMES = [
    { id: 'cs2', name: 'Counter-Strike 2', icon: '🎯', platform: 'FACEIT' }
] as const;

export type GameId = typeof SUPPORTED_GAMES[number]['id'];
export type MatchMode = 'bo1' | 'bo3';
