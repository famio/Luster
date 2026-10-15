import { defineConfig } from 'vite';

export default defineConfig({
  // The package is linked from web/luster, which has its own three for its
  // checks; the page draws with one.
  resolve: { dedupe: ['three'] },
  server: { fs: { allow: ['../..'] } },
  build: { chunkSizeWarningLimit: 800 },
});
