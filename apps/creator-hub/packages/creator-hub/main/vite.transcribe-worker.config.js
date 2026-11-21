import config from './vite.worker.config.js';

/**
 * The session recorder's transcription worker, built like the optimizer worker as its own
 * single file; `@fugood/whisper.node` is downloaded on first use and stays external.
 * @type {import('vite').UserConfig}
 */
export default {
  ...config,
  build: {
    ...config.build,
    lib: { ...config.build.lib, entry: 'src/transcribe-worker.ts' },
    rolldownOptions: {
      ...config.build.rolldownOptions,
      external: ['@fugood/whisper.node'],
    },
  },
};
