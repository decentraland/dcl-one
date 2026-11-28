import config from './vite.worker.config.js';

/**
 * The deploy runner (src/deploy-runner.ts), built like the optimizer worker as its own single
 * file. It loads the scene's own `@dcl/sdk-commands` at runtime, so nothing is external.
 * @type {import('vite').UserConfig}
 */
export default {
  ...config,
  build: {
    ...config.build,
    lib: { ...config.build.lib, entry: 'src/deploy-runner.ts' },
    rolldownOptions: {
      ...config.build.rolldownOptions,
      external: [],
    },
  },
};
