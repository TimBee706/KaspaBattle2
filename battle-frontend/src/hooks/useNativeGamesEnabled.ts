import { create } from 'zustand';
import { useEffect } from 'react';
import { getFeatures } from '../api/nativeGames';

interface FeatureState {
    loaded: boolean;
    nativeGamesEnabled: boolean;
    setNativeGamesEnabled: (enabled: boolean) => void;
}

const useFeatureStore = create<FeatureState>((set) => ({
    loaded: false,
    nativeGamesEnabled: false,
    setNativeGamesEnabled: (nativeGamesEnabled) => set({ nativeGamesEnabled, loaded: true }),
}));

/** `VITE_NATIVE_GAMES_ENABLED=false` force-hides native games regardless of the backend. */
const FORCE_HIDDEN = import.meta.env.VITE_NATIVE_GAMES_ENABLED?.trim().toLowerCase() === 'false';

let requested = false;

/**
 * Whether the backend has NATIVE_GAMES_ENABLED on (GET /features). Fails closed: while loading or
 * if the request fails, native games are treated as disabled for *creating* matches.
 */
export function useNativeGamesEnabled(): boolean {
    const { loaded, nativeGamesEnabled, setNativeGamesEnabled } = useFeatureStore();
    useEffect(() => {
        if (loaded || requested) return;
        requested = true;
        getFeatures()
            .then((f) => setNativeGamesEnabled(!!f.nativeGamesEnabled))
            .catch(() => {
                requested = false;
                setNativeGamesEnabled(false);
            });
    }, [loaded, setNativeGamesEnabled]);
    return !FORCE_HIDDEN && nativeGamesEnabled;
}

/** Test helper. */
export function __setNativeGamesEnabledForTests(enabled: boolean) {
    useFeatureStore.getState().setNativeGamesEnabled(enabled);
}
