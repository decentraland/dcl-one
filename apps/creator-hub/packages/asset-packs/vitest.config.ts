import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  resolve: {
    alias: {
      // `~sdk/script-utils` is a virtual module the scene bundle provides at build time; stub it in
      // tests so `actions.ts` (which imports `callScriptMethod` from it) can be exercised.
      '~sdk/script-utils': fileURLToPath(new URL('./test/mocks/script-utils.ts', import.meta.url)),
      // `~system/*` are SDK host modules provided by the scene runtime; stub them for the same reason.
      '~system/RestrictedActions': fileURLToPath(
        new URL('./test/mocks/system.ts', import.meta.url),
      ),
      '~system/SignedFetch': fileURLToPath(new URL('./test/mocks/system.ts', import.meta.url)),
      '~system/Runtime': fileURLToPath(new URL('./test/mocks/system.ts', import.meta.url)),
      '~system/EngineApi': fileURLToPath(new URL('./test/mocks/system.ts', import.meta.url)),
      '~system/CommsApi': fileURLToPath(new URL('./test/mocks/system.ts', import.meta.url)),
      '~system/CommunicationsController': fileURLToPath(
        new URL('./test/mocks/system.ts', import.meta.url),
      ),
      '~system/UserIdentity': fileURLToPath(new URL('./test/mocks/system.ts', import.meta.url)),
      // The installed @dcl/sdk has no `cast` module yet (tsconfig.sdk-cast.json): the tests run the
      // tree's source.
      '@dcl/sdk/cast': fileURLToPath(
        new URL(
          '../../../../library/js-sdk-toolchain/packages/@dcl/sdk/src/cast/index.ts',
          import.meta.url,
        ),
      ),
    },
    // The tree's source has no node_modules of its own.
    dedupe: ['@dcl/ecs'],
  },
});
