import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
// @ts-expect-error -- plain JS helper, shared with the two vite configs so the
import { validateAlias } from "./vite.validate.js";

export default defineConfig({
  resolve: {
    alias: [
      ...validateAlias(),
      { find: "@catalyst-api", replacement: fileURLToPath(new URL("../../library/catalyst-api", import.meta.url)) },
      { find: "@shop", replacement: fileURLToPath(new URL("../../library/shop", import.meta.url)) },
    ],
    // library/shop has no node_modules of its own: its viem imports resolve to ui3's copy.
    dedupe: ["viem"],
  },
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    // HUD tests render the whole shell with lazy chunks; the default 5 s fails under shared-machine load.
    testTimeout: 15000,
    execArgv: ["--no-experimental-webstorage"],
    projects: [
      {
        extends: true,
        test: {
          name: "ui3",
          include: ["src/**/*.test.{ts,tsx}"],
          exclude: ["src/play/**"],
          setupFiles: ["src/test/setup.ts"],
          css: true,
        },
      },
      {
        // The Shop's portable core (library/shop), which every Shop host shares.
        extends: true,
        test: {
          name: "shop-core",
          dir: "../../library/shop",
          include: ["**/*.test.ts"],
          environment: "node",
        },
      },
      {
        // The Explorer HUD (src/play) under the harness it was written for upstream
        // (react-web/vitest.config.ts): its own setup file, no CSS processing.
        extends: true,
        test: {
          name: "play",
          include: ["src/play/**/*.test.{ts,tsx}"],
          setupFiles: ["src/play/test/setup.ts"],
          css: false,
        },
      },
    ],
  },
});
