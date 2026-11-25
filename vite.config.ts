/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { dirname } from "node:path";

const host = process.env.TAURI_DEV_HOST;

// `VITE_DEMO_SNAPSHOT=<file> npm run dev` (src/lib/demo.ts): the browser UI shows sessions the app made, so the
// dev server may serve the snapshot and the folders its pictures are in, and nothing else outside the project.
const snapshot = process.env.VITE_DEMO_SNAPSHOT;
function snapshotFolders(file: string): string[] {
  const folders = new Set([dirname(file)]);
  const walk = (v: unknown): void => {
    if (typeof v === "string") {
      if (/^\/.+\.[A-Za-z0-9]{2,5}$/.test(v)) folders.add(dirname(v));
    } else if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === "object") Object.values(v).forEach(walk);
  };
  const data = JSON.parse(readFileSync(file, "utf8"));
  walk(data);
  // The snapshot writes the home folder as /Users/demo (src/lib/demo.ts).
  return [...folders].map((f) => f.replace(/^\/Users\/demo(?=\/)/, data.home));
}

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
    fs: snapshot ? { allow: [".", ...snapshotFolders(snapshot)] } : undefined,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari15",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "node",
  },
});
