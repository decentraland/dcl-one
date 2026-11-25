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

// Vite inlines a bare `import.meta.env` as the whole env object, every build-time `VITE_*` value,
// into the release bundle. Reading single keys (`import.meta.env.X`) inlines only those.
describe('the renderer source', () => {
  it('should never read import.meta.env as a whole object', () => {
    const root = join(__dirname, '..');
    const files = readdirSync(root, { recursive: true, encoding: 'utf8' }).filter(
      file => /\.tsx?$/.test(file) && !/\.(spec|test)\.tsx?$/.test(file),
    );
    const bare = files.filter(file =>
      readFileSync(join(root, file), 'utf8')
        .split('\n')
        .some(line => !/^\s*(\/\/|\*)/.test(line) && /import\.meta\.env(?!\.\w)/.test(line)),
    );

    expect(bare).toEqual([]);
  });
});
