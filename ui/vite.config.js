import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// API port when running UI dev server (npm run dev). API runs at 8080 locally, 80 in Docker.
const apiTarget = process.env.VITE_API_TARGET || 'http://localhost:8080'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: true,
      },
    },
  },
})
