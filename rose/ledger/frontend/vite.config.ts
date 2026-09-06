import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// In dev the Vite server owns the browser and proxies /api to the Rust backend
// on :47000, so the frontend and API feel like one origin without CORS fuss.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": "http://localhost:47000",
    },
  },
});
