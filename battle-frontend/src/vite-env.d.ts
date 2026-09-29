/// <reference types="vite/client" />

interface ImportMetaEnv {
    readonly VITE_KASPA_NODE_URL: string;
    readonly VITE_KASPA_NETWORK: string;
    readonly VITE_KASPA_PUBLIC_FALLBACK?: string;
    readonly VITE_API_BASE_URL: string;
    readonly VITE_WS_BASE_URL: string;
    readonly VITE_FACEIT_CLIENT_ID: string;
    readonly VITE_FACEIT_REDIRECT_URI: string;
    /** 'false' force-hides native browser games in the UI (default: follow the backend flag). */
    readonly VITE_NATIVE_GAMES_ENABLED?: string;
}

interface ImportMeta {
    readonly env: ImportMetaEnv;
}
