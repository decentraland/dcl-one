import { readdirSync, readFileSync } from 'fs';
import { join } from 'path';

import { describe, expect, it } from 'vitest';

import { devAiSharingUrl } from './aiSharingDevUrl';

describe('devAiSharingUrl', () => {
  describe('when a dev build has VITE_AI_SHARING_API_URL', () => {
    it('should return it, trimmed', () => {
      expect(
        devAiSharingUrl({ DEV: true, VITE_AI_SHARING_API_URL: ' http://localhost:3000 ' }),
      ).toBe('http://localhost:3000');
    });
  });

  describe('when a dev build has no override', () => {
    it('should return nothing', () => {
      expect(devAiSharingUrl({ DEV: true })).toBe('');
      expect(devAiSharingUrl({ DEV: true, VITE_AI_SHARING_API_URL: '  ' })).toBe('');
    });
  });

  describe('when the build is packaged or a release', () => {
    it('should ignore the variable even if it was set at build time', () => {
      expect(
        devAiSharingUrl({ DEV: false, VITE_AI_SHARING_API_URL: 'http://localhost:3000' }),
      ).toBe('');
    });
  });
});

// Vite inlines a bare or computed `import.meta.env` (`import.meta.env[key]`) as the whole env
// object, every build-time `VITE_*` value, into the release bundle. Reading single keys
// (`import.meta.env.X`) inlines only those.
const sourcesOf = (root: string) =>
  readdirSync(root, { recursive: true, encoding: 'utf8' })
    .filter(file => /\.tsx?$/.test(file) && !/\.(spec|test)\.tsx?$/.test(file))
    .map(file => join(root, file));
const readsWholeEnv = (file: string, pattern: RegExp) =>
  readFileSync(file, 'utf8')
    .split('\n')
    .some(line => !/^\s*(\/\/|\*)/.test(line) && pattern.test(line));

describe('the renderer source', () => {
  it('should never read import.meta.env as a whole object', () => {
    const bare = sourcesOf(join(__dirname, '..')).filter(file =>
      readsWholeEnv(file, /import\.meta\.env(?!\.\w)/),
    );

    expect(bare).toEqual([]);
  });
});

// The renderer bundles library/web-core (`@web-core`), which reads `import.meta.env?.X`.
describe('the shared web-core source the renderer bundles', () => {
  it('should never read import.meta.env as a whole object or by a computed key', () => {
    const webCore = join(__dirname, '../../../../../../../library/web-core');
    const bare = sourcesOf(webCore).filter(file =>
      readsWholeEnv(file, /(?<!typeof )import\.meta\.env(?!\??\.\w)/),
    );

    expect(sourcesOf(webCore).length).toBeGreaterThan(0);
    expect(bare).toEqual([]);
  });
});
