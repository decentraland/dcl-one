// @vitest-environment node
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import ts from 'typescript'
import { describe, expect, it } from 'vitest'

// What ships to every visitor of the overview: the modules src/main.tsx reaches without an import() boundary.
// Local imports are followed one by one; an npm package is checked through its declared dependencies instead,
// which over-reports: a declared dependency the bundler drops has to be listed below.
const ROOT = process.cwd()
const SRC = resolve(ROOT, 'src')
const ENTRY = resolve(SRC, 'main.tsx')

const FORBIDDEN_PACKAGES = [/^viem(\/|$)/, /^@dcl\/crypto(\/|$)/, /^decentraland-connect(\/|$)/, /^jszip(\/|$)/]
const FORBIDDEN_FILES = [/^src\/lib\/chain\//]
// Exact specifiers: the package's deep imports are fine, its root barrel is not.
const FORBIDDEN_SPECIFIERS = [/^@dcl\/schemas$/]
// Declared dependencies the first load never loads. Imported for types only (checked below):
const TYPE_ONLY_DEPENDENCIES: Record<string, string[]> = { '@dcl/single-sign-on-client': ['@dcl/crypto'] }
// Tree-shaken, which nothing here can check: only @dcl/hooks' notifications client imports it, and no
// decentraland-ui2 component on the first load uses that client. Using a new ui2 component on the overview or in
// the shell means checking the built first-load chunks for @dcl/crypto again.
const TREE_SHAKEN_DEPENDENCIES: Record<string, string[]> = { '@dcl/hooks': ['decentraland-crypto-fetch'] }

const packageName = (specifier: string) =>
  specifier
    .split('/')
    .slice(0, specifier.startsWith('@') ? 2 : 1)
    .join('/')

function packageDir(from: string, name: string): string | null {
  for (let dir = from; ; dir = dirname(dir)) {
    if (existsSync(join(dir, 'node_modules', name, 'package.json'))) return join(dir, 'node_modules', name)
    if (dirname(dir) === dir) return null
  }
}

function resolveLocal(from: string, specifier: string): string {
  const path = specifier.replace(/\?.*$/, '')
  const base = path.startsWith('~/') ? resolve(SRC, path.slice(2)) : resolve(dirname(from), path)
  for (const candidate of [base, `${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`]) {
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate
  }
  throw new Error(`Cannot resolve ${specifier} from ${relative(ROOT, from)}`)
}

// Type-only imports and re-exports are erased before bundling, so they load nothing.
function isTypeOnly(statement: ts.ImportDeclaration | ts.ExportDeclaration): boolean {
  if (ts.isImportDeclaration(statement)) {
    const clause = statement.importClause
    if (!clause) return false
    if (clause.isTypeOnly) return true
    const bindings = clause.namedBindings
    return (
      !clause.name &&
      !!bindings &&
      ts.isNamedImports(bindings) &&
      bindings.elements.length > 0 &&
      bindings.elements.every(element => element.isTypeOnly)
    )
  }
  if (statement.isTypeOnly) return true
  const clause = statement.exportClause
  return !!clause && ts.isNamedExports(clause) && clause.elements.length > 0 && clause.elements.every(e => e.isTypeOnly)
}

function staticImports(file: string): string[] {
  const kind = file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS
  const source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, false, kind)
  return source.statements.flatMap(statement =>
    (ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement)) &&
    statement.moduleSpecifier &&
    ts.isStringLiteral(statement.moduleSpecifier) &&
    !isTypeOnly(statement)
      ? [statement.moduleSpecifier.text]
      : []
  )
}

/** Every module reachable from the entry, with the chain of importers that reaches it first. */
function firstLoadGraph() {
  const paths = new Map<string, string[]>([[relative(ROOT, ENTRY), [relative(ROOT, ENTRY)]]])
  const queue = [ENTRY]
  while (queue.length) {
    const file = queue.shift()!
    const path = paths.get(relative(ROOT, file))!
    if (!/\.tsx?$/.test(file)) continue
    for (const specifier of staticImports(file)) {
      const local = specifier.startsWith('.') || specifier.startsWith('~/')
      const target = local ? resolveLocal(file, specifier) : specifier
      const key = local ? relative(ROOT, target) : specifier
      if (paths.has(key)) continue
      paths.set(key, [...path, key])
      if (local) queue.push(target)
    }
  }
  return paths
}

/** Every package the first load's npm imports depend on, peers included, with the chain that reaches it. */
function packageClosure(graph: Map<string, string[]>) {
  const packages = new Map<string, string[]>()
  const queue = [...graph.entries()]
    .filter(([key]) => !key.startsWith('src/'))
    .map(([key, path]) => ({ from: ROOT, name: packageName(key), path }))
  while (queue.length) {
    const { from, name, path } = queue.shift()!
    // An optional peer that isn't installed loads nothing.
    const dir = packageDir(from, name)
    if (!dir || packages.has(dir)) continue
    packages.set(dir, path)
    const { dependencies = {}, peerDependencies = {} } = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'))
    for (const dependency of Object.keys({ ...dependencies, ...peerDependencies })) {
      if ([TYPE_ONLY_DEPENDENCIES, TREE_SHAKEN_DEPENDENCIES].some(skip => skip[name]?.includes(dependency))) continue
      queue.push({ from: dir, name: dependency, path: [...path, dependency] })
    }
  }
  return packages
}

function builtFiles(dir: string): string[] {
  return readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .filter(file => !file.split('/').includes('node_modules') && /\.[cm]?js$/.test(file))
    .map(file => join(dir, file))
}

describe("the overview's first load", () => {
  const graph = firstLoadGraph()

  it('follows the shell down to the navbar and the session restore', () => {
    expect(graph.has('src/components/NavBar/NavBar.tsx')).toBe(true)
    expect(graph.has('src/lib/auth/auth.ts')).toBe(true)
    expect(graph.has('@dcl/single-sign-on-client')).toBe(true)
  })

  it('loads no wallet client, signing, curve or chain code', () => {
    const leaks = [...graph.entries()]
      .filter(([key]) =>
        [...FORBIDDEN_PACKAGES, ...FORBIDDEN_FILES, ...FORBIDDEN_SPECIFIERS].some(pattern => pattern.test(key))
      )
      .map(([, path]) => path.join(' > '))
    expect(leaks).toEqual([])
  })

  it('reaches no wallet client, signing, curve or chain package through its npm dependencies', () => {
    const leaks = [...packageClosure(graph).values()]
      .filter(path => FORBIDDEN_PACKAGES.some(pattern => pattern.test(path[path.length - 1])))
      .map(path => path.join(' > '))
    expect(leaks).toEqual([])
  })

  it.each(Object.entries(TYPE_ONLY_DEPENDENCIES))(
    'imports nothing at runtime from what %s uses for types',
    (name, types) => {
      const imports = builtFiles(packageDir(ROOT, name)!).filter(file => {
        const code = readFileSync(file, 'utf8')
        return types.some(type => new RegExp(`['"]${type}['"/]`).test(code))
      })
      expect(imports).toEqual([])
    }
  )
})
