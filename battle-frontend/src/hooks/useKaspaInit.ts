import { useState, useEffect } from 'react';
import { initKaspaWasm, getInitStatus } from '../kaspa/init';

export function useKaspaInit() {
    const [isReady, setIsReady] = useState(getInitStatus());
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (isReady) return;
        initKaspaWasm()
            .then(() => setIsReady(true))
            .catch((err) => setError(`WASM-Initialisierung fehlgeschlagen: ${err.message}`));
    }, [isReady]);

    return { isReady, error };
}
