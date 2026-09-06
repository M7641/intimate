import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// Proxy API calls to the Axum backend so the browser talks same-origin and we
// avoid CORS entirely in dev.
export default defineConfig({
  plugins: [solid()],
  server: {
    proxy: {
      "/api": "http://localhost:3000",
      "/health": "http://localhost:3000",
    },
  },
});
