import * as kaspa from 'kaspa-wasm';

let initPromise: Promise<void> | null = null;
let isInitialized = false;

export async function initKaspaWasm(): Promise<void> {
    if (isInitialized) return;
    if (initPromise) return initPromise;

    initPromise = (async () => {
        try {
            console.log("TEST_DEBUG: Calling kaspa.default()...");
            // @ts-ignore - Some bundlers need precise path resolution or omit the argument entirely
            await kaspa.default();
            console.log("✅ Kaspa Web SDK erfolgreich initialisiert!");
            isInitialized = true;
        } catch (e) {
            console.error("❌ Fehler beim Initialisieren des Kaspa Web SDK!", e);
            throw e;
        }
    })();

    return initPromise;
}

export function getInitStatus(): boolean {
    return isInitialized;
}
