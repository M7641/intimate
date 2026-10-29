import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";

const root = fileURLToPath(new URL("..", import.meta.url));
const engineDirs = ["core", "wasm", "data"].map((d) => `${root}${d}/`);

// The engine and its data are compiled into public/atlantis.wasm. In dev,
// rebuild it when they change, then reload the page.
function rebuildEngine(): Plugin {
  return {
    name: "rebuild-engine",
    apply: "serve",
    configureServer(server) {
      server.watcher.add(engineDirs);
      let timer: ReturnType<typeof setTimeout> | undefined;
      let building = false;
      let again = false;
      const build = () => {
        if (building) {
          again = true;
          return;
        }
        building = true;
        server.config.logger.info("engine changed: rebuilding wasm", { timestamp: true });
        const child = spawn("bash", [`${root}scripts/build-wasm.sh`], { stdio: "inherit" });
        child.on("exit", (code) => {
          building = false;
          if (code === 0) {
            server.ws.send({ type: "full-reload" });
          } else {
            server.config.logger.error("wasm build failed; page not reloaded", { timestamp: true });
          }
          if (again) {
            again = false;
            build();
          }
        });
      };
      server.watcher.on("change", (file) => {
        if (!engineDirs.some((d) => file.startsWith(d))) return;
        clearTimeout(timer);
        timer = setTimeout(build, 200);
      });
    },
  };
}

export default defineConfig({
  plugins: [solid(), rebuildEngine()],
  // The card browser imports artwork from ../assets, outside this root.
  server: { port: 5180, fs: { allow: [".."] } },
  // One HTML file per page: the game, and the card browser.
  build: { outDir: "dist", rolldownOptions: { input: ["index.html", "cards.html"] } },
});
