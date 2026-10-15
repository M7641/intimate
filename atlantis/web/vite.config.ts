import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  // The card browser imports artwork from ../assets, outside this root.
  server: { port: 5180, fs: { allow: [".."] } },
  // One HTML file per page: the game, and the card browser.
  build: { outDir: "dist", rolldownOptions: { input: ["index.html", "cards.html"] } },
});
