import { defineConfig } from "@playwright/test";
import { browserLaunchOptions } from "./scripts/browser-options.mjs";
export default defineConfig({
  outputDir: "./test-results/playwright",
  testDir: "./tests/browser", timeout: 120000, workers: 1,
  use: { baseURL: "http://127.0.0.1:4173", browserName: "chromium", launchOptions: browserLaunchOptions },
  webServer: { command: "node scripts/serve.mjs", url: "http://127.0.0.1:4173/tests/browser/host.html", reuseExistingServer: !process.env.CI },
});
