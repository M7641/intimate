import os from "os";
import path from "path";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tsconfigPaths from "vite-tsconfig-paths";

const TENANT = process.env.TENANT || "";

// Get workspace ID from hostname
const hostname = os.hostname() || "";
const workspaceId = hostname.includes("workspace-")
  ? hostname.split("workspace-")[1].slice(0, -2)
  : "";
const userName = process.env.USER || "";

// https://vite.dev/config/
export default defineConfig({
  server: {
    host: "localhost",
    port: 5173,
    allowedHosts: [`${TENANT}-${workspaceId}.nimbus.example`],
    proxy: {
      "/api": {
        target: hostname.includes("workspace-")
          ? `https://${TENANT}-${workspaceId}.nimbus.example`
          : "http://localhost:8050",
      },
    },
  },
  build: { outDir: "dist" },
  plugins: [
    tsconfigPaths(),
    tailwindcss(),
    react({
      babel: {
        plugins: ["babel-plugin-react-compiler"],
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  base: hostname.includes("workspace-")
    ? `/user/${userName}/proxy/absolute/5173`
    : "/",
});
