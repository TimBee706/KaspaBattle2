import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
    plugins: [react()],
    test: {
        globals: true,
        environment: 'jsdom',
        setupFiles: './src/__tests__/setup.ts',
        include: ['src/__tests__/**/*.test.{ts,tsx}'],
        coverage: {
            provider: 'v8',
            reporter: ['text', 'html'],
            include: ['src/**/*.{ts,tsx}'],
            exclude: ['src/__tests__/**', 'src/vite-env.d.ts'],
        },
    },
    server: {
        allowedHosts: [
            'nonminimal-secernent-ernestina.ngrok-free.dev',
            '.ngrok-free.app'  // Wildcard für zukünftige ngrok‑Tunnels
        ]
    }
});
