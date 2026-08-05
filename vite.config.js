import { defineConfig } from 'vite';

export default defineConfig({
  root: 'src',
  base: './',
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
  },
});
