import path from "path"
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/zz/api": {
        target: "http://127.0.0.1:9090",
        changeOrigin: true,
      },
      "/zz/ws": {
        target: "ws://127.0.0.1:9090",
        ws: true,
      },
    },
  },
})
