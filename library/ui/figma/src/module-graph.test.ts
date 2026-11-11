import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'
import { describe, expect, it } from 'vitest'
import { componentModulePaths, isComponentModulePath } from '../component-generation.mjs'
import { moduleSpecifiers } from '../module-graph.mjs'

const repository = execFileSync('git', ['rev-parse', '--show-toplevel'], {
  cwd: import.meta.dirname
})
  .toString()
  .trim()
const scope = 'library/ui/figma/'
const tracked = execFileSync('git', ['-C', repository, 'ls-files', '-z', '--', scope])
  .toString()
  .split('\0')
  .filter(Boolean)
const read = (file: string) => readFileSync(path.join(repository, file))

describe('when the dependency-free import reader scans the tracked Figma tooling', () => {
  const candidates = tracked.filter(
    (file) => /\.(?:mjs|js|mts|ts)$/.test(file) && isComponentModulePath(file)
  )

  it('should find some candidate modules to compare', () => {
    expect(candidates.length).toBeGreaterThan(50)
  })

  it.each(candidates)('should list the same specifiers as the TypeScript parser for %s', (file) => {
    const text = read(file).toString('utf8')
    const parsed = ts
      .preProcessFile(text, true, true)
      .importedFiles.map((entry) => entry.fileName)
      .sort()
    expect(moduleSpecifiers(text, file, true).sort()).toEqual(parsed)
  })
})

describe('when the component read-set is selected from the committed source lock', () => {
  const lock = JSON.parse(read('library/ui/upstream.lock.json').toString('utf8')) as {
    sources: { repository: string; files: Record<string, { sha256: string }> }[]
  }
  const source = lock.sources.find((entry) =>
    entry.repository.endsWith('/decentraland/figma-design-system')
  )!
  const paths = componentModulePaths(source, read)

  it.each([
    'src/install-main.ts',
    'src/install.ts',
    'src/install-components.ts',
    'src/app-context.ts',
    'src/sources/snapshot-source.ts',
    'src/generators/components.ts',
    'src/generators/naming.ts',
    'src/specs.ts',
    'module-graph.mjs',
    'ledger/retirements.json'
  ])('should bind %s, which the install process loads or reads', (file) => {
    expect(paths).toContain(scope + file)
  })

  it.each([
    'src/cli.ts',
    'src/app.ts',
    'src/port.ts',
    'src/server.ts',
    'src/sync.ts',
    'src/contract/resolve.ts',
    'src/tools/index.ts',
    'src/tools/contract.ts',
    'key-guard.mjs',
    'lock-digests.mjs',
    'port-verification.mjs',
    'public-components.mjs'
  ])('should leave %s out, which the install process never loads', (file) => {
    expect(tracked).toContain(scope + file)
    expect(paths).not.toContain(scope + file)
  })

  it('should match the module list of the committed generation record', () => {
    const record = JSON.parse(read('library/ui/components/generation.json').toString('utf8')) as {
      modules: { path: string }[]
    }
    expect(record.modules.map((module) => module.path)).toEqual(paths)
  })
})

describe('when the install process runs with a hook that reports every module it loads', () => {
  const lock = JSON.parse(read('library/ui/upstream.lock.json').toString('utf8')) as {
    sources: { repository: string; files: Record<string, { sha256: string }> }[]
  }
  const source = lock.sources.find((entry) =>
    entry.repository.endsWith('/decentraland/figma-design-system')
  )!
  const hook =
    'data:text/javascript,' +
    encodeURIComponent(
      "import { registerHooks } from 'node:module'\n" +
        'registerHooks({ load(url, context, next) {\n' +
        "  if (url.startsWith('file:') && !url.includes('node_modules')) process.stderr.write('LOADED ' + url + '\\n')\n" +
        '  return next(url, context)\n' +
        '} })\n'
    )
  // An unknown target fails after every static import has loaded and before anything is read or written.
  const run = () => {
    try {
      execFileSync(
        process.execPath,
        [
          '--import',
          import.meta.resolve('tsx'),
          '--import',
          hook,
          path.join(import.meta.dirname, 'install-main.ts'),
          'unknown'
        ],
        { cwd: path.join(import.meta.dirname, '..'), stdio: 'pipe' }
      )
    } catch (error) {
      return String((error as { stderr: Buffer }).stderr)
    }
    throw new Error('The install process accepted an unknown target')
  }

  it('should load exactly the code modules the static closure binds', () => {
    const output = run()
    const root = path.join(repository, scope)
    const loaded = output
      .split('\n')
      .filter((line) => line.startsWith('LOADED '))
      .map((line) => fileURLToPath(line.slice('LOADED '.length)))
      .map((file) => scope + path.relative(root, file).split(path.sep).join('/'))
      .sort()
    expect(output).toContain('Unknown install target')
    expect(loaded).toEqual(
      componentModulePaths(source, read).filter(
        (file) => /\.(?:mjs|ts)$/.test(file) && !file.includes('/review/')
      )
    )
  })
})
