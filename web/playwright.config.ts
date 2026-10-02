import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";
const macChrome =
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
export default defineConfig({
  testDir: "./tests",
  workers: 1,
  timeout: 30000,
  use: {
    baseURL: "http://127.0.0.1:3000",
    headless: true,
    launchOptions: {
      executablePath:
        process.env.CHROME_PATH ||
        (existsSync(macChrome) ? macChrome : undefined),
    },
    screenshot: "only-on-failure",
  },
  reporter: "list",
});
