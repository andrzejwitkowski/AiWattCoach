import { loadEnv } from 'vite';
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const backendProxyTarget = env.BACKEND_PROXY_TARGET || 'http://127.0.0.1:3002';

  return {
    plugins: [react(), tailwindcss()],
    server: {
      port: 5173,
      proxy: {
        '/api': {
          target: backendProxyTarget,
          changeOrigin: true,
          ws: true,
        },
        '/health': backendProxyTarget,
        '/ready': backendProxyTarget
      }
    },
    test: {
      environment: 'jsdom',
      setupFiles: './src/test/setup.ts'
    }
  };
});
