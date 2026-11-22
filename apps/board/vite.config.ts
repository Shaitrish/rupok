import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { fileURLToPath } from "node:url";

export default defineConfig({
  // Served from the domain root: "/" is the website, "/board/" is the app.
  base: "/",
  plugins: [solid()],
  server: {
    port: 3737,
    host: "127.0.0.1",
    fs: { allow: ["../.."] },
  },
  build: {
    target: "es2022",
    chunkSizeWarningLimit: 4096,
    rollupOptions: {
      input: {
        site: fileURLToPath(new URL("./index.html", import.meta.url)),
        board: fileURLToPath(new URL("./board/index.html", import.meta.url)),
      },
    },
  },
  optimizeDeps: { exclude: ["./src/wasm/rupok.js"] },
});
