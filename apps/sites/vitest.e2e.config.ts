import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const ui = fileURLToPath(new URL("../ui3/src", import.meta.url));
const src = fileURLToPath(new URL("./src", import.meta.url));
const catalystApi = fileURLToPath(new URL("../../library/catalyst-api", import.meta.url));
const cssStub = fileURLToPath(new URL("./test/e2e/css-stub.ts", import.meta.url));

export default defineConfig({
  resolve: {
    alias: [
      { find: /^.*\.css$/, replacement: cssStub },
      { find: "@ui", replacement: ui },
      { find: "@catalyst-api", replacement: catalystApi },
      { find: "dcl-validate-impl", replacement: `${ui}/validate/checked.ts` },
      { find: "@", replacement: src },
    ],
    dedupe: ["react", "react-dom", "react-router"],
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
