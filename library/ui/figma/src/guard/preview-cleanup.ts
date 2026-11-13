import { realpath } from 'node:fs/promises'
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import path from 'node:path'
import { promisify } from 'node:util'
import { createApp } from '../app-context.js'
import { generateAll } from '../generators/index.js'
import { createReviewManifest } from '../review-manifest.js'
import { diffFiles, ownedFiles } from '../sync.js'
import { parseAggregate } from './aggregates.js'
import { readTree, findConsumers, type Tree } from './consumers.js'
import { listFiles, readBytesIfExists, resolveContained, toPosix } from './fs-safe.js'
import { buildOwnership, GENERATED, SNAPSHOT } from './ownership.js'
import ts from 'typescript'

const execute = promisify(execFile)
const CONFIG = 'figma/figma.config.json'
const CATALOG = 'figma/review/src/review/catalog.json'
const AGGREGATES = [
  [`${GENERATED}/components/index.ts`, 'components-index'],
  [`${GENERATED}/components/manifest.ts`, 'components-manifest'],
  [CATALOG, 'catalog']
] as const
const sha256 = (value: Buffer | string) => createHash('sha256').update(value).digest('hex')
const removablePath = (file: string) =>
  /^components\/(?:assets\/[a-f0-9]+\.svg|[A-Za-z0-9_-]+\/[A-Za-z][A-Za-z0-9]*\.(?:tsx|module\.css))$/.test(
    file
  )

async function bytes(root: string, file: string) {
  return readBytesIfExists(await resolveContained(root, file))
}

async function unchangedDirectory(base: string, staged: string, directory: string) {
  const files = new Set<string>()
  for (const root of [base, staged])
    for (const file of await listFiles(await resolveContained(root, directory)))
      files.add(toPosix(path.relative(root, file)))
  for (const file of files) {
    const before = await bytes(base, file)
    const after = await bytes(staged, file)
    if (!before || !after || !before.equals(after))
      throw new Error(`Preview cleanup cannot change protected input: ${file}`)
  }
}

async function gitContext(root: string) {
  const repository = (
    await execute('git', ['-C', root, 'rev-parse', '--show-toplevel'])
  ).stdout.trim()
  const head = (await execute('git', ['-C', root, 'rev-parse', 'HEAD'])).stdout.trim()
  const skipped = (await execute('git', ['-C', repository, 'ls-files', '-t', '-z'])).stdout
    .split('\0')
    .find((entry) => entry.startsWith('S '))
  if (skipped)
    throw new Error('Preview cleanup requires a full checkout to audit every tracked consumer')
  const prefix = toPosix(path.relative(await realpath(repository), await realpath(root)))
  if (prefix.startsWith('..'))
    throw new Error('Preview cleanup needs a repository-contained library')
  const committed = async (relative: string) => {
    const file = prefix ? `${prefix}/${relative}` : relative
    try {
      return (
        await execute('git', ['-C', repository, 'show', `${head}:${file}`], {
          encoding: 'buffer',
          maxBuffer: 64 * 1024 * 1024
        })
      ).stdout
    } catch {
      throw new Error(`Preview output has no committed provenance: ${relative}`)
    }
  }
  return { repository, head, prefix, committed }
}

function storedDynamicSpecifiers(file: string, text: string): string[] {
  if (!/\bimport(?:\s|\/\*[\s\S]*?\*\/)*\(/.test(text)) return []
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true)
  const imports: ts.Expression[] = []
  const collect = (node: ts.Node) => {
    if (
      ts.isCallExpression(node) &&
      node.expression.kind === ts.SyntaxKind.ImportKeyword &&
      node.arguments[0]
    )
      imports.push(node.arguments[0])
    ts.forEachChild(node, collect)
  }
  collect(source)
  if (!imports.length) return []
  const options: ts.CompilerOptions = { noLib: true, noResolve: true, allowJs: true }
  const host = ts.createCompilerHost(options)
  host.getSourceFile = (name) => (name === file ? source : undefined)
  host.fileExists = (name) => name === file
  host.readFile = (name) => (name === file ? text : undefined)
  const checker = ts.createProgram([file], options, host).getTypeChecker()
  // Property accesses may instantiate fresh symbols; declarations retain their lexical identity.
  const bindings = new Map<ts.Declaration, ts.Expression[]>()
  const symbolAt = (node: ts.Node) =>
    ts.isElementAccessExpression(node) && ts.isStringLiteralLike(node.argumentExpression)
      ? checker.getPropertyOfType(
          checker.getTypeAtLocation(node.expression),
          node.argumentExpression.text
        )
      : checker.getSymbolAtLocation(node)
  const bind = (name: ts.Node, value: ts.Expression) => {
    const symbol = symbolAt(name)
    for (const declaration of symbol?.declarations ?? [])
      bindings.set(declaration, [...(bindings.get(declaration) ?? []), value])
  }
  const visit = (node: ts.Node) => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer)
      bind(node.name, node.initializer)
    if (ts.isPropertyAssignment(node)) bind(node.name, node.initializer)
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken)
      bind(node.left, node.right)
    ts.forEachChild(node, visit)
  }
  visit(source)
  const values = (node: ts.Expression, seen: ReadonlySet<ts.Declaration>): string[] => {
    if (ts.isStringLiteralLike(node)) return [node.text]
    if (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node) ||
      ts.isTypeAssertionExpression(node) ||
      ts.isNonNullExpression(node)
    )
      return values(node.expression, seen)
    const declarations = symbolAt(node)?.declarations ?? []
    return declarations.flatMap((declaration) => {
      if (seen.has(declaration)) return []
      const next = new Set([...seen, declaration])
      return (bindings.get(declaration) ?? []).flatMap((value) => values(value, next))
    })
  }
  return imports.flatMap((node) => values(node, new Set()))
}

function importedTargets(tree: Tree, repository: string, excludedNames: ReadonlySet<string>) {
  const configurations = new Map<string, ts.CompilerOptions>()
  const compilerOptions = (file: string) => {
    const found = ts.findConfigFile(path.dirname(file), ts.sys.fileExists)
    if (!found)
      return { moduleResolution: ts.ModuleResolutionKind.Bundler, module: ts.ModuleKind.ESNext }
    const cached = configurations.get(found)
    if (cached) return cached
    const parsed = ts.getParsedCommandLineOfConfigFile(
      found,
      {},
      {
        ...ts.sys, // Module identity needs options, not a second project-wide file scan.
        readDirectory: () => [],
        onUnRecoverableConfigFileDiagnostic: () => {}
      }
    )
    const options = parsed?.options ?? {
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      module: ts.ModuleKind.ESNext
    }
    configurations.set(found, options)
    return options
  }
  const resolveImport = (file: string, specifier: string) => {
    const absolute = path.join(repository, file)
    const resolved = ts.resolveModuleName(specifier, absolute, compilerOptions(absolute), ts.sys)
      .resolvedModule?.resolvedFileName
    if (resolved) return toPosix(path.relative(repository, resolved))
    const name = path.posix.basename(specifier).replace(/\.[cm]?[jt]sx?$/, '')
    if (excludedNames.has(name))
      throw new Error(
        `Cannot prove an unresolved module is unrelated to excluded Preview output: ${file} (${specifier})`
      )
    return undefined
  }
  const references = new Map<string, Set<string>>()
  const add = (target: string, consumer: string) => {
    const previous = references.get(target) ?? new Set<string>()
    previous.add(consumer)
    references.set(target, previous)
  }
  const resolve = (file: string, specifier: string) => {
    if (!specifier.startsWith('.')) return undefined
    const target = path.posix.normalize(path.posix.join(path.posix.dirname(file), specifier))
    return [target, `${target}.ts`, `${target}.tsx`, `${target}/index.ts`].find((candidate) =>
      tree.paths.has(candidate)
    )
  }
  for (const [file, text] of tree.texts) {
    if (/\.[cm]?[jt]sx?$/.test(file)) {
      const specifiers = new Set([
        ...ts.preProcessFile(text, true, true).importedFiles.map((entry) => entry.fileName),
        ...storedDynamicSpecifiers(path.join(repository, file), text)
      ])
      for (const specifier of specifiers) {
        const target = resolve(file, specifier) ?? resolveImport(file, specifier)
        if (target) add(target, file)
      }
    }
    if (file.endsWith('.css'))
      for (const match of text.matchAll(/(?:url\(\s*['"]?([^'"\s)]+)|\bfrom\s+['"]([^'"]+))/g)) {
        const target = resolve(file, match[1] ?? match[2]!)
        if (target) add(target, file)
      }
  }
  return references
}

// Integrity tables describe artifact bytes. Only recognized fingerprint pairs are bookkeeping; other fields still count.
function withoutIntegrityPairs(text: string) {
  const fingerprint = (value: unknown) => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value)
  const table = (value: unknown) =>
    value && typeof value === 'object' && !Array.isArray(value)
      ? Object.fromEntries(
          Object.entries(value).filter(
            ([, record]) =>
              !fingerprint(record) &&
              !(
                record &&
                typeof record === 'object' &&
                fingerprint((record as { sha256?: unknown }).sha256)
              )
          )
        )
      : value
  const data = JSON.parse(text)
  if (Array.isArray(data.sources))
    for (const source of data.sources) {
      source.files = table(source.files)
      if (source.snapshot) source.snapshot.digests = table(source.snapshot.digests)
    }
  for (const field of ['publicFoundations', 'publicComponents'])
    if (data[field]) data[field].files = table(data[field].files)
  return JSON.stringify(data)
}

/** Recomputes exact derivative removals; a caller supplies no paths or identity-retirement claims. */
async function previewCleanupPlan(base: string, staged: string, requirePruned = false) {
  const app = await createApp(path.join(base, 'figma'), await resolveContained(base, CONFIG))
  const source = app.createSource('snapshot')
  await source.ready?.()
  if (path.resolve(app.cwd, app.config.snapshotDir) !== path.join(base, SNAPSHOT))
    throw new Error('Preview cleanup requires the repository capture directory')
  for (const directory of [SNAPSHOT, 'figma/ledger', 'components'])
    await unchangedDirectory(base, staged, directory)
  const lock =
    app.config.snapshotLock &&
    toPosix(path.relative(base, path.resolve(app.cwd, app.config.snapshotLock)))
  if (!lock || lock.startsWith('../'))
    throw new Error('Preview cleanup needs a contained capture lock')
  for (const file of [CONFIG, lock, 'index.ts']) {
    const before = await bytes(base, file)
    const after = await bytes(staged, file)
    if (!before || !after || !before.equals(after))
      throw new Error(`Preview cleanup cannot change protected input: ${file}`)
  }
  const fresh = await generateAll(source, {}, app.config)
  if (fresh.summary.components === 0)
    throw new Error('Preview cleanup refuses an empty ready component generation')
  const expected = new Set(fresh.files.map((file) => file.path))
  if (!expected.has('components/index.ts') || !expected.has('components/manifest.ts'))
    throw new Error('Preview cleanup refuses an incomplete generation')
  const output = await resolveContained(staged, GENERATED)
  const diff = await diffFiles(output, fresh.files)
  if (diff.added.length || diff.changed.length)
    throw new Error('Preview cleanup requires complete freshly synchronized output')
  const expectedCatalog = await createReviewManifest({ ...app, resolveOutDir: () => output })
  const catalog = await bytes(staged, CATALOG)
  if (
    !catalog ||
    JSON.stringify(JSON.parse(catalog.toString('utf8'))) !== JSON.stringify(expectedCatalog)
  )
    throw new Error('Preview cleanup requires the complete freshly derived Review catalog')

  const git = await gitContext(base)
  const baselineLock = JSON.parse((await git.committed('upstream.lock.json')).toString('utf8')) as {
    sources?: { repository?: string; files?: Record<string, { sha256?: string }> }[]
  }
  const pins = baselineLock.sources?.find(
    (source) => source.repository === 'https://github.com/decentraland/figma-design-system'
  )?.files
  if (!pins || !Object.keys(pins).length)
    throw new Error('Preview cleanup needs committed output pins')
  const excluded = (await ownedFiles(path.join(base, GENERATED))).filter(
    (file) => !expected.has(file)
  )
  const removed = new Set(excluded.map((file) => `${GENERATED}/${file}`))
  for (const relative of excluded) {
    const file = `${GENERATED}/${relative}`
    if (!removablePath(relative))
      throw new Error(`Preview cleanup cannot remove an unowned path: ${relative}`)
    const current = await bytes(base, file)
    const original = await git.committed(file)
    const key = git.prefix ? `${git.prefix}/${file}` : file
    if (!current || !current.equals(original) || pins[key]?.sha256 !== sha256(original))
      throw new Error(`Preview output is edited or lacks pinned historical provenance: ${relative}`)
    if (relative.endsWith('.tsx') || relative.endsWith('.module.css')) {
      const module = relative.replace(/\.module\.css$/, '.tsx')
      const owner = await git.committed(`${GENERATED}/${module}`)
      if (!owner.toString('utf8').startsWith('// Generated by dcl-figma '))
        throw new Error(`Preview output has no generated module owner: ${relative}`)
    }
  }
  if (requirePruned && diff.removed.length)
    throw new Error('Preview cleanup left stale output in its stage')
  if (!requirePruned && diff.removed.some((file) => !removed.has(`${GENERATED}/${file}`)))
    throw new Error('Preview cleanup found an unexpected staged artifact')

  const tree = await readTree(git.repository)
  const repositoryPath = (file: string) => (git.prefix ? `${git.prefix}/${file}` : file)
  const lockPath = repositoryPath('upstream.lock.json')
  const lockText = tree.texts.get(lockPath)
  if (lockText) tree.texts.set(lockPath, withoutIntegrityPairs(lockText))
  for (const ledger of ['figma/ledger/orphaned-assets.json', 'figma/ledger/retirements.json'])
    tree.texts.delete(repositoryPath(ledger))
  const ownedPrefix = `${repositoryPath(GENERATED)}/`
  for (const file of [...tree.paths])
    if (file.startsWith(ownedPrefix)) {
      tree.paths.delete(file)
      tree.texts.delete(file)
    }
  for (const file of fresh.files) {
    const relative = repositoryPath(`${GENERATED}/${file.path}`)
    tree.paths.add(relative)
    if (/\.(?:tsx?|css|json)$/.test(file.path)) tree.texts.set(relative, file.content)
  }
  tree.texts.set(repositoryPath(CATALOG), catalog.toString('utf8'))
  // Keep excluded paths resolvable while finding references, without keeping their obsolete source as consumers.
  for (const file of removed) tree.paths.add(repositoryPath(file))
  const references = removed.size
    ? importedTargets(
        tree,
        git.repository,
        new Set(
          excluded
            .filter((file) => file.endsWith('.tsx'))
            .map((file) => path.posix.basename(file, '.tsx'))
        )
      )
    : new Map<string, Set<string>>()
  const modules = new Map(
    [...expected]
      .filter((file) => file.endsWith('.tsx'))
      .map((file) => [path.posix.basename(file, '.tsx'), file])
  )
  const excludedModules = new Map(
    excluded
      .filter((file) => file.endsWith('.tsx'))
      .map((file) => [path.posix.basename(file, '.tsx'), file])
  )
  for (const file of fresh.files.filter((file) => file.path.endsWith('.tsx')))
    for (const match of file.content.matchAll(/\bref:\s*\{\s*component:\s*['"]([^'"]+)['"]/g)) {
      const target = excludedModules.get(match[1]!)
      if (target && !modules.has(match[1]!))
        throw new Error(`Ready preview still references excluded component: ${target}`)
    }
  for (const file of removed) {
    const target = repositoryPath(file)
    const consumers = file.endsWith('.svg')
      ? findConsumers(tree, target).map((consumer) => consumer.file)
      : [...(references.get(target) ?? [])]
    if (consumers.length)
      throw new Error(`Preview output still has a surviving consumer: ${file} (${consumers[0]})`)
  }
  const artifacts = new Set(removed)
  const beforeOwnership = await buildOwnership(base)
  const afterOwnership = await buildOwnership(staged)
  for (const [file, kind] of AGGREGATES) {
    const before = await bytes(base, file)
    const after = await bytes(staged, file)
    const previous = before && parseAggregate(kind, before.toString('utf8'), beforeOwnership)
    const next = after && parseAggregate(kind, after.toString('utf8'), afterOwnership)
    if (!next) throw new Error(`Fresh preview aggregate is missing: ${file}`)
    for (const key of previous?.entries.keys() ?? [])
      if (!next.entries.has(key)) artifacts.add(`${file}#${key}`)
  }
  return { head: git.head, removed: [...removed].sort(), artifacts }
}

export { previewCleanupPlan }
