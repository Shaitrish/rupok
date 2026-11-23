// End-to-end flows of the board (apps/board) in WebKit: the engine of Safari,
// of the macOS app's WKWebView and of the Linux app's WebKitGTK. CI runs it
// on macOS (.github/workflows/desktop.yml, job "WebKit e2e").
//
//   cd apps/board && bun run vendor && bun run build   # dist/ with rupoklib
//   cd e2e && bun install && bunx playwright install webkit
//   bunx playwright test                               # serves dist/ itself
//
// Set E2E_BROWSERS=webkit,chromium to compare with Chromium.

import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.E2E_PORT || 4791);
const BROWSERS: Record<string, (typeof devices)[string]> = {
  webkit: devices["Desktop Safari"],
  chromium: devices["Desktop Chrome"],
};
const wanted = (process.env.E2E_BROWSERS || "webkit").split(",").map((s) => s.trim());

export default defineConfig({
  testDir: ".",
  timeout: 60_000,
  expect: { timeout: 15_000 },
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  outputDir: "test-results",
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    viewport: { width: 1280, height: 800 },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: wanted.map((name) => ({ name, use: { ...BROWSERS[name], viewport: { width: 1280, height: 800 } } })),
  // The production build, as on rupok.app and in the desktop app.
  webServer: {
    command: `npx vite preview --host 127.0.0.1 --port ${PORT} --strictPort`,
    cwd: "..",
    url: `http://127.0.0.1:${PORT}/board/`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
