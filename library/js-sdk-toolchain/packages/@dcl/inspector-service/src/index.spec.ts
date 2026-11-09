// @vitest-environment node
import { builtinModules } from 'module';
import { build } from 'esbuild';

import { getNotBundledModules } from '../build-externals.js';

describe('when bundling the service for Node', () => {
  let runtimeRequires: string[];

  beforeEach(async () => {
    const result = await build({
      entryPoints: ['src/index.ts'],
      bundle: true,
      platform: 'node',
      write: false,
      metafile: true,
      define: { INSPECTOR_DEV_PARSER: 'false' },
      external: getNotBundledModules(),
      loader: { '.glb': 'dataurl' },
      logLevel: 'silent',
    });
    runtimeRequires = Object.values(result.metafile.outputs).flatMap(output =>
      output.imports.filter(i => i.external).map(i => i.path),
    );
  }, 60_000);

  it('should bundle the renderer math dependencies instead of requiring them at runtime', () => {
    expect(runtimeRequires.filter(path => /^@dcl\/ecs-math/.test(path))).toEqual([]);
  });

  it('should require nothing but node and the optional native accelerators at runtime', () => {
    const builtin = new Set(builtinModules.concat(builtinModules.map($ => `node:${$}`)));
    const optionalNative = ['bufferutil', 'utf-8-validate'];
    const packages = runtimeRequires.filter(path => !builtin.has(path));
    expect(packages.sort()).toEqual(optionalNative.sort());
  });
});
