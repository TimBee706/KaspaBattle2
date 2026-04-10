// Kaspa (Testnet Config)
const rawKaspaNodeUrl = import.meta.env.VITE_KASPA_NODE_URL?.trim();

export const KASPA_NETWORK = import.meta.env.VITE_KASPA_NETWORK || 'testnet-12';
// KASPA_NODE_URL: set to empty string to use the Resolver (may have CORS issues in browser).
// Set a direct WSS URL to bypass the Resolver's HTTP discovery step entirely.
export const KASPA_NODE_URL = rawKaspaNodeUrl ?? '';
export const KASPA_EXPLORER_URL = 'https://explorer-tn12.kaspa.org'; // Testnet-12 Explorer
export const SOMPI_PER_KAS = 100_000_000; // 1 KAS = 10^8 Sompi

// KaspaBattle API
// Uses relative paths by default so Vite Proxy handles CORS cleanly!
export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api/v1';
export const WS_BASE_URL = import.meta.env.VITE_WS_BASE_URL || '/ws';

// FACEIT OAuth
// Client ID is public but must be set via VITE_FACEIT_CLIENT_ID env variable
export const FACEIT_CLIENT_ID = import.meta.env.VITE_FACEIT_CLIENT_ID ?? '';
export const FACEIT_REDIRECT_URI = import.meta.env.VITE_FACEIT_REDIRECT_URI ?? 'http://localhost:5173/auth/faceit/callback';
export const FACEIT_AUTH_URL = 'https://accounts.faceit.com';
export const FACEIT_SCOPES = 'openid profile email';

// Match-Limits (aus Whitepaper)
export const MIN_WAGER_KAS = 10;
export const MAX_WAGER_KAS = 10_000;
export const MIN_WAGER_SOMPI = MIN_WAGER_KAS * SOMPI_PER_KAS;
export const MAX_WAGER_SOMPI = MAX_WAGER_KAS * SOMPI_PER_KAS;

// Fee-Struktur
export const FEE_WINNER_PERCENT = 99;   // 99% an den Gewinner
export const FEE_TREASURY_PERCENT = 1;  // 1% Protokoll-Fee
export const FEE_ORACLE_PERCENT = 0;    // Oracle-Reserve bis Phase 2 in Treasury konsolidiert

// Polling
export const MATCH_STATUS_POLL_INTERVAL_MS = 5_000;
export const LOBBY_POLL_INTERVAL_MS = 10_000;

// Unterstützte Spiele
export const SUPPORTED_GAMES = [
    { id: 'cs2', name: 'Counter-Strike 2', icon: '/game_logo_cs2.svg', platform: 'FACEIT' },
    { id: 'valorant', name: 'Valorant', icon: '/game_logo_valorant.svg', platform: 'FACEIT' },
    { id: 'rocket_league', name: 'Rocket League', icon: '/game_logo_rocket_league.svg', platform: 'FACEIT' },
    { id: 'dota2', name: 'Dota 2', icon: '/game_logo_dota2.svg', platform: 'FACEIT' },
    { id: 'lol', name: 'League of Legends', icon: '/game_logo_lol.svg', platform: 'FACEIT' }
] as const;

export type GameId = typeof SUPPORTED_GAMES[number]['id'];
export type MatchMode = 'bo1' | 'bo3';
