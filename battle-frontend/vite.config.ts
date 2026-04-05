import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react(),
    wasm(),
    topLevelAwait(),
  ],
  assetsInclude: ['**/*.wasm'],
  optimizeDeps: {
    exclude: [], // Erlaube Vite die Optimierung/Konvertierung zu ESM
  },
  build: {
    target: 'esnext',
  },
  server: {
    port: 3000,
    allowedHosts: true, // Erlaubt Ngrok und andere Tunnel
    fs: {
      allow: ['..']
    },
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://localhost:8080',
        ws: true,
      },
    }
  }
});
