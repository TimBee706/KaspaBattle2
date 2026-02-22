let initPromise: Promise<void> | null = null;
let isInitialized = false;

export async function initKaspaWasm(): Promise<void> {
    if (isInitialized) return;
    if (initPromise) return initPromise;

    initPromise = (async () => {
        const kaspa = await import('kaspa-wasm');
        await kaspa.default();  // WASM initialisieren
        isInitialized = true;
    })();

    return initPromise;
}

export function getInitStatus(): boolean {
    return isInitialized;
}
