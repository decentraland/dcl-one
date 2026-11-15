// @vitest-environment node
import { readFileSync } from 'node:fs';
import { builtinModules } from 'node:module';
import { build } from 'esbuild';

import { getNotBundledModules } from '../build-externals';

describe('when bundling the tooling entrypoint for Node', () => {
  let runtimeRequires: string[];

  beforeEach(async () => {
    const result = await build({
      entryPoints: ['src/tooling-entrypoint.ts'],
      bundle: true,
      platform: 'node',
      write: false,
      metafile: true,
      external: getNotBundledModules(),
      loader: { '.glb': 'dataurl' },
      logLevel: 'silent',
    });
    runtimeRequires = Object.values(result.metafile.outputs).flatMap(output =>
      output.imports.filter(i => i.external).map(i => i.path),
    );
  }, 60_000);

  it('should bundle the renderer math dependencies instead of requiring them at runtime', () => {
    expect(runtimeRequires.filter(path => /^(@babylonjs\/|@dcl\/ecs-math)/.test(path))).toEqual([]);
  });

  it('should require at runtime only Node built-ins and packages that the manifest has in dependencies', () => {
    const { dependencies } = JSON.parse(readFileSync('package.json', 'utf8'));
    const packageOf = (path: string) =>
      path
        .split('/')
        .slice(0, path.startsWith('@') ? 2 : 1)
        .join('/');
    const notInstalled = runtimeRequires.filter(
      path =>
        !path.startsWith('node:') &&
        !builtinModules.includes(path) &&
        !(packageOf(path) in dependencies),
    );
    expect([...new Set(notInstalled)]).toEqual([]);
  });
});
