import { createRequire } from 'node:module'
import { mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'

const require = createRequire(import.meta.url)
const { buildSync } = createRequire(require.resolve('@dcl/sdk-commands/package.json'))('esbuild')
const output = mkdtempSync(join(tmpdir(), 'editor-scene-tests-'))
const testNames = readdirSync(new URL('.', import.meta.url)).filter(f => f.endsWith('.test.ts')).map(f => f.slice(0, -'.test.ts'.length)).sort()
try {
  for (const name of testNames) {
    const file = join(output, `${name}.cjs`)
    buildSync({
      entryPoints: [new URL(`./${name}.test.ts`, import.meta.url).pathname],
      outfile: file, bundle: true, platform: 'node', format: 'cjs', logLevel: 'warning'
    })
    const result = spawnSync(process.execPath, [file], { stdio: 'inherit' })
    if (result.error) throw result.error
    if (result.status !== 0) {
      process.exitCode = result.status ?? 1
      break
    }
  }
  if (!process.exitCode) {
    const result = spawnSync(process.execPath, [new URL('./login.test.mjs', import.meta.url).pathname], { stdio: 'inherit' })
    if (result.error) throw result.error
    process.exitCode = result.status ?? 1
  }
} finally {
  rmSync(output, { recursive: true, force: true })
}
