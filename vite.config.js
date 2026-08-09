import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process is a nodejs global
const commit = process.env.GITHUB_SHA ? process.env.GITHUB_SHA.slice(0, 7) : "dev";

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [sveltekit()],

  // 注入构建时的 commit ID（GitHub Actions 提供 GITHUB_SHA；本地为 "dev"）
  define: {
    __APP_COMMIT__: JSON.stringify(commit),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
