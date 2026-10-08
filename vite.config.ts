import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri ожидает стабильный порт в dev-режиме
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "chrome105",
    outDir: "dist",
  },
});
