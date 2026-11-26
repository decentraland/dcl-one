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

  // Every import, re-export and literal dynamic import in the TypeScript syntax tree.
  const parsedSpecifiers = (text: string, file: string) => {
    const found: string[] = []
    const visit = (node: ts.Node) => {
      if (
        (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
        node.moduleSpecifier &&
        ts.isStringLiteral(node.moduleSpecifier)
      )
        found.push(node.moduleSpecifier.text)
      else if (
        ts.isCallExpression(node) &&
        node.expression.kind === ts.SyntaxKind.ImportKeyword &&
        node.arguments[0] &&
        ts.isStringLiteral(node.arguments[0])
      )
        found.push(node.arguments[0].text)
      ts.forEachChild(node, visit)
    }
    visit(ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true))
    return found.sort()
  }

  // Every identifier named require or getBuiltinModule, and every createRequire that is not the imported binding or
  // the callee of an immediate .resolve() call: the loaders the reader must refuse.
  const parsedLoaders = (text: string, file: string) => {
    const found: string[] = []
    const visit = (node: ts.Node) => {
      if (ts.isIdentifier(node) && ['require', 'getBuiltinModule'].includes(node.text))
        found.push(node.text)
      if (ts.isIdentifier(node) && node.text === 'createRequire') {
        const call = node.parent
        const sanctioned =
          ts.isImportSpecifier(call) ||
          (ts.isCallExpression(call) &&
            call.expression === node &&
            ts.isPropertyAccessExpression(call.parent) &&
            call.parent.name.text === 'resolve' &&
            ts.isCallExpression(call.parent.parent))
        if (!sanctioned) found.push(node.text)
      }
      ts.forEachChild(node, visit)
    }
    visit(ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true))
    return found
  }

  it.each(candidates)('should find no module loader the reader refuses in %s', (file) => {
    expect(parsedLoaders(read(file).toString('utf8'), file)).toEqual([])
  })

  it.each(candidates)('should list the same specifiers as the TypeScript parser for %s', (file) => {
    const text = read(file).toString('utf8')
    expect(moduleSpecifiers(text, file, true).sort()).toEqual(parsedSpecifiers(text, file))
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
  // The process fails after every static import has loaded and before anything is read or written.
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
    // The hook is a preload the install process refuses, after every static import has loaded.
    expect(output).toContain('runs only with the tsx loader')
    expect(loaded).toEqual(
      componentModulePaths(source, read).filter(
        (file) => /\.(?:mjs|ts)$/.test(file) && !file.includes('/review/')
      )
    )
  })
})
