import { defineConfig } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// GitHub Pages serves this project from a sub-path
// (/elohim-gate-viewer/), where a root-absolute asset URL 404s. Setting
// PAGES=1 switches the build to relative asset URLs. It is opt-in rather than
// always on because the Tauri build loads index.html over a custom protocol at
// the root, and that path is not something I can verify by running the web
// build -- so the default stays exactly as it was.
const pages = Boolean(process.env.PAGES);

// https://vite.dev/config/
export default defineConfig(() => ({
  base: pages ? "./" : "/",

  build: {
    rollupOptions: {
      input: {
        // The desktop app.
        main: "index.html",
        // The published report. Same bundle, one more entry, so the report can
        // never drift from the viewer it shares styles and helpers with.
        report: "report.html",
      },
    },
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
