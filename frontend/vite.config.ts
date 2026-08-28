import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 3050,
    proxy: {
      '/api': 'http://localhost:3055',
      '/auth': 'http://localhost:3055',
      '/health': 'http://localhost:3055',
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (id.includes('node_modules/antd')) return 'antd';
          if (id.includes('node_modules/react')) return 'vendor';
          if (id.includes('node_modules/dayjs')) return 'vendor';
          if (id.includes('node_modules/@ant-design')) return 'antd';
        },
      },
    },
  },
});