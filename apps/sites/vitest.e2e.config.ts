import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const ui = fileURLToPath(new URL("../ui3/src", import.meta.url));
const src = fileURLToPath(new URL("./src", import.meta.url));
const catalystApi = fileURLToPath(new URL("../../library/catalyst-api", import.meta.url));
const shopCore = fileURLToPath(new URL("../../library/shop", import.meta.url));
const webCore = fileURLToPath(new URL("../../library/web-core", import.meta.url));
const cssStub = fileURLToPath(new URL("./test/e2e/css-stub.ts", import.meta.url));

export default defineConfig({
  resolve: {
    alias: [
      { find: /^.*\.css$/, replacement: cssStub },
      { find: "@ui", replacement: ui },
      { find: "@dcl/one-ui", replacement: fileURLToPath(new URL("../../library/ui", import.meta.url)) },
      { find: "@catalyst-api", replacement: catalystApi },
      { find: "@shop", replacement: shopCore },
      { find: "@web-core", replacement: webCore },
      { find: "dcl-validate-impl", replacement: `${webCore}/validate/checked.ts` },
      { find: "@", replacement: src },
    ],
    // viem, zod: library/shop and library/web-core have no node_modules of their own, so their imports
    // resolve to this app's copy.
    dedupe: ["react", "react-dom", "react-router", "viem", "zod", "@walletconnect/sign-client"],
  },
  test: {
    include: ["test/e2e/**/*.e2e.test.ts"],
    globalSetup: ["./test/e2e/globalSetup.ts"],
    environment: "node",
    testTimeout: 30_000,
    hookTimeout: 90_000,
    fileParallelism: false,
    pool: "forks",
  },
});
