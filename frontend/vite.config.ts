import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const frontendDir = fileURLToPath(new URL('./', import.meta.url));

export default defineConfig({
  root: frontendDir,
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(frontendDir, 'src'),
      '@public': resolve(frontendDir, 'public'),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        settings: resolve(frontendDir, 'settings.html'),
        translate: resolve(frontendDir, 'translate.html'),
        ocr: resolve(frontendDir, 'ocr.html'),
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.js'],
  },
});
