// Runs the scene's unit tests in node. Each test file is bundled with the scene modules it imports, by the esbuild that
// sdk-commands builds the scene with, and node's test runner runs the bundles. The SDK's runtime entry (`@dcl/sdk`,
// which src/index.ts re-exports) and the runtime's `~system/*` modules exist only inside the engine and no test calls
// them, so they are bundled as empty modules; `@dcl/sdk/ecs` and `@dcl/sdk/math` are the real packages.
import { createRequire } from 'node:module'
import { mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const require = createRequire(import.meta.url)
const sdk = createRequire(require.resolve('@dcl/sdk/package.json'))
const { build } = createRequire(sdk.resolve('@dcl/sdk-commands/package.json'))('esbuild')

const engineOnly = {
  name: 'engine-only-modules',
  setup(b) {
    b.onResolve({ filter: /^(@dcl\/sdk|~system\/.+)$/ }, (a) => ({ path: a.path, namespace: 'engine-only' }))
    b.onLoad({ filter: /.*/, namespace: 'engine-only' }, () => ({ contents: 'export {}', loader: 'js' }))
  }
}

const dir = fileURLToPath(new URL('.', import.meta.url))
const tests = readdirSync(dir).filter((f) => f.endsWith('.test.ts')).sort()
const output = mkdtempSync(join(tmpdir(), 'movement-scene-tests-'))
try {
  const bundles = []
  for (const test of tests) {
    const outfile = join(output, test.replace(/\.ts$/, '.cjs'))
    await build({
      entryPoints: [join(dir, test)], outfile, bundle: true, platform: 'node', format: 'cjs', target: 'node20',
      sourcemap: 'inline', logLevel: 'warning', plugins: [engineOnly]
    })
    bundles.push(outfile)
  }
  const result = spawnSync(process.execPath, ['--enable-source-maps', '--test', ...bundles], { stdio: 'inherit' })
  if (result.error) throw result.error
  process.exitCode = result.status ?? 1
} finally {
  rmSync(output, { recursive: true, force: true })
}
