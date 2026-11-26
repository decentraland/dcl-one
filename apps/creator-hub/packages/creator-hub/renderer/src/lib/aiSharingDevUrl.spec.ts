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

// Vite inlines `import.meta.env.X` (or `?.X`) as that one value, but any other read of the env
// object (bare, `[key]`, `["X"]`, `import.meta["env"]`) as the whole object: every build-time
// `VITE_*` value goes into the release bundle. Checked on whole files with comments removed, so a
// read split across lines or cast (`(import.meta as …).env`) is caught too.
const SOURCE = /\.(tsx?|mts|jsx?|mjs)$/;
const sourcesOf = (root: string) =>
  readdirSync(root, { recursive: true, encoding: 'utf8' })
    .filter(
      file =>
        SOURCE.test(file) &&
        !/\.(spec|test)\.\w+$/.test(file) &&
        !/(^|[/\\])(node_modules|dist)[/\\]/.test(file),
    )
    .map(file => join(root, file));

const ENV_ACCESS =
  /import\s*\.\s*meta(?:\s+as\s+[^)]*)?\s*\)?\s*(?:\?\.\s*|\.\s*)(?:env\b|\[\s*['"`]env['"`]\s*\])|import\s*\.\s*meta\s*(?:\?\.)?\s*\[\s*['"`]env['"`]\s*\]/g;
const STATIC_KEY = /^\s*(?:\?\.|\.)\s*[A-Za-z_$]/;

const wholeEnvReads = (source: string) => {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:\\])\/\/.*$/gm, '$1');
  return [...code.matchAll(ENV_ACCESS)]
    .filter(m => /\[/.test(m[0]) || !STATIC_KEY.test(code.slice(m.index + m[0].length)))
    .map(m => m[0]);
};

describe('the env read check', () => {
  it('should allow reading one key', () => {
    expect(
      wholeEnvReads(
        'a = import.meta.env.VITE_X; b = import.meta.env?.VITE_Y; c = import.meta?.env?.DEV;\n' +
          'd = (import.meta as { env?: { DEV?: boolean } }).env?.DEV; if (typeof import.meta) {}',
      ),
    ).toEqual([]);
  });

  it('should flag every read of the whole env object', () => {
    const reads = [
      'const e = import.meta.env;',
      'f(import.meta.env)',
      'import.meta.env[key]',
      'import.meta.env?.[key]',
      'import.meta.env["VITE_X"]',
      'import.meta?.env[key]',
      '(import.meta as any).env[key]',
      'import.meta["env"]',
      'import.meta\n  .env[key]',
      'typeof import.meta.env',
    ];

    for (const read of reads) expect(wholeEnvReads(read), read).toHaveLength(1);
  });
});

// Everything the renderer bundles: its own source, `/shared`, and the `@ui`, `@shop`,
// `@web-core` and `@dcl/one-ui` aliases (vite.config.js). All of ui3 is checked: its Sites
// builds leak the same way.
describe.each([
  ['apps/creator-hub/packages/creator-hub/renderer/src'],
  ['apps/creator-hub/packages/creator-hub/shared'],
  ['apps/ui3/src'],
  ['library/shop'],
  ['library/web-core'],
  ['library/ui'],
])('the source in %s', dir => {
  it('should read import.meta.env one key at a time', () => {
    const root = join(__dirname, '../../../../../../..', dir);
    const files = sourcesOf(root);
    const leaks = files
      .map(file => ({ file, reads: wholeEnvReads(readFileSync(file, 'utf8')) }))
      .filter(({ reads }) => reads.length > 0);

    expect(files.length).toBeGreaterThan(0);
    expect(leaks).toEqual([]);
  });
});
