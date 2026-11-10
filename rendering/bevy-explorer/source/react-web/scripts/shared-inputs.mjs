import { Buffer } from 'node:buffer'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync
} from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const OWNED_SOURCE = 'rendering/bevy-explorer/source/'
export const WEB = OWNED_SOURCE + 'react-web'
export const MANIFEST = WEB + '/shared-inputs.json'
const GENERATOR = WEB + '/scripts/shared-inputs.mjs'
const GENERATING = Symbol.for('dcl.hud.shared-inputs.generating')
const CONTROL_PATHS = [
  GENERATOR,
  WEB + '/package.json',
  WEB + '/package-lock.json',
  WEB + '/tsconfig.json',
  WEB + '/vite.config.ts',
  WEB + '/vitest.config.ts',
  WEB + '/vite.shared.ts',
  'library/ui/package.json',
  'library/ui/foundations/typography/font-source.json'
]
const HERE = path.dirname(fileURLToPath(import.meta.url))
const REPOSITORY = path.resolve(HERE, '../../../../..')
const SHARED = ['library/ui/', 'library/shop/', 'library/catalyst-api/']
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const relative = (root, file) => path.relative(root, file).split(path.sep).join('/')
const portable = (value) =>
  typeof value === 'string' &&
  value.length > 0 &&
  !value.startsWith('/') &&
  !value.includes('\\') &&
  ![...value].some((character) => character.charCodeAt(0) < 32) &&
  value.split('/').every((part) => part && part !== '.' && part !== '..')
const shared = (file) =>
  !file.split('/').includes('node_modules') && SHARED.some((prefix) => file.startsWith(prefix))
const sorted = (values) => [...values].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
const git = (root, ...args) =>
  execFileSync('git', ['--no-replace-objects', '--literal-pathspecs', '-C', root, ...args], {
    maxBuffer: 1 << 28
  })
const hasGit = (root) => existsSync(path.join(root, '.git'))
function rejectAutomaticPostcss(root) {
  const names = new Set([
    '.postcssrc',
    '.postcssrc.json',
    '.postcssrc.yaml',
    '.postcssrc.yml',
    '.postcssrc.js',
    '.postcssrc.cjs',
    '.postcssrc.mjs',
    '.postcssrc.ts',
    '.postcssrc.cts',
    '.postcssrc.mts',
    'postcss.config.js',
    'postcss.config.cjs',
    'postcss.config.mjs',
    'postcss.config.ts',
    'postcss.config.cts',
    'postcss.config.mts'
  ])
  let ancestor = root
  for (const part of WEB.split('/')) {
    ancestor = path.join(ancestor, part)
    const entry = lstatSync(ancestor)
    if (entry.isSymbolicLink() || !entry.isDirectory())
      throw Error('Unsafe HUD configuration directory: ' + relative(root, ancestor))
  }
  let directory = path.join(root, WEB)
  for (;;) {
    if (readdirSync(directory).some((name) => names.has(name)))
      throw Error('Automatic PostCSS configuration is unsupported by the checked HUD contract')
    const packageFile = path.join(directory, 'package.json')
    if (
      existsSync(packageFile) &&
      Object.hasOwn(JSON.parse(regular(root, relative(root, packageFile))), 'postcss')
    )
      throw Error(
        'Automatic package PostCSS configuration is unsupported by the checked HUD contract'
      )
    if (directory === root) return
    directory = path.dirname(directory)
  }
}
function sourceInventory(root, declared = []) {
  if (hasGit(root))
    return new Set(git(root, 'ls-files', '-z').toString().split('\0').filter(Boolean))
  const files = new Set()
  const walk = (relative) => {
    const directory = path.join(root, relative)
    if (!existsSync(directory)) return
    let parent = root
    for (const part of relative.split('/')) {
      parent = path.join(parent, part)
      const entry = lstatSync(parent)
      if (entry.isSymbolicLink() || !entry.isDirectory())
        throw Error('Unsafe filtered-source directory: ' + relative)
    }
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (
        entry.name === 'node_modules' ||
        relative + '/' + entry.name === WEB + '/src/engine/generated'
      )
        continue
      const file = relative + '/' + entry.name
      if (entry.isDirectory() && !entry.isSymbolicLink()) walk(file)
      else files.add(file)
    }
  }
  for (const prefix of [WEB, ...SHARED.map((value) => value.slice(0, -1))]) walk(prefix)
  for (const file of declared) {
    if (!portable(file)) throw Error('Unsafe filtered-source control path')
    files.add(file)
  }
  return files
}
export function rootInventory(paths) {
  return sorted(
    [...paths].filter(
      (file) =>
        file.startsWith(WEB + '/src/') || file === WEB + '/app.html' || file === WEB + '/index.html'
    )
  )
}
function resolverControls(paths, inputs, physicalRoot) {
  const result = new Set()
  for (const file of inputs) {
    let directory = path.posix.dirname(file)
    while (directory) {
      for (const name of ['package.json', 'tsconfig.json', 'jsconfig.json']) {
        const candidate = (directory === '.' ? '' : directory + '/') + name
        if (
          paths.has(candidate) ||
          (physicalRoot && lstatSync(path.join(physicalRoot, candidate), { throwIfNoEntry: false }))
        )
          result.add(candidate)
      }
      if (directory === '.') break
      directory = path.posix.dirname(directory)
    }
  }
  return result
}
function regular(root, file) {
  if (!portable(file)) throw Error('Unsafe shared input path: ' + file)
  let current = root
  for (const part of file.split('/')) {
    current = path.join(current, part)
    if (lstatSync(current).isSymbolicLink()) throw Error('Linked shared input: ' + file)
  }
  if (!lstatSync(current).isFile()) throw Error('Non-file shared input: ' + file)
  return readFileSync(current)
}
function identity(root, file, paths, expectedLfs) {
  if (!paths.has(file)) throw Error('Missing or untracked shared import or control: ' + file)
  const bytes = regular(root, file)
  if (!hasGit(root) && expectedLfs) {
    if (sha256(bytes) !== expectedLfs.oid || bytes.length !== expectedLfs.size)
      throw Error('Unverified filtered-source LFS payload: ' + file)
    return { path: file, sha256: expectedLfs.oid, lfs: expectedLfs }
  }
  const index = hasGit(root) ? git(root, 'show', ':' + file).toString() : ''
  if (index.startsWith('version https://git-lfs.github.com/spec/v1\n')) {
    const match =
      /^version https:\/\/git-lfs.github.com\/spec\/v1\noid sha256:([a-f0-9]{64})\nsize (\d+)\n$/.exec(
        index
      )
    if (!match || sha256(bytes) !== match[1] || bytes.length !== Number(match[2]))
      throw Error('Unverified materialized shared LFS input: ' + file)
    return {
      path: file,
      sha256: match[1],
      lfs: { oid: match[1], size: Number(match[2]) }
    }
  }
  if (bytes.subarray(0, 43).toString().startsWith('version https://git-lfs.github.com/spec/'))
    throw Error('Unmaterialized shared input: ' + file)
  return { path: file, sha256: sha256(bytes) }
}

/** Dev/native consumers read the same checked contract as CI, Nix and fingerprinting. */
export function readSharedPaths(root = REPOSITORY, { mode = 'strict' } = {}) {
  if (!['strict', 'config'].includes(mode)) throw Error('Invalid HUD shared-input read mode')
  const strict = mode === 'strict'
  if (globalThis[GENERATING] > 0) return []
  rejectAutomaticPostcss(root)
  const manifest = JSON.parse(regular(root, MANIFEST).toString())
  if (
    Object.keys(manifest).sort().join(',') !== 'controls,edges,files,roots,schema' ||
    manifest.schema !== 1 ||
    !Array.isArray(manifest.roots) ||
    !Array.isArray(manifest.controls) ||
    !Array.isArray(manifest.files) ||
    !Array.isArray(manifest.edges)
  )
    throw Error('Invalid HUD shared-input contract')
  const paths = strict
    ? sourceInventory(
        root,
        [...manifest.controls, ...manifest.files].map((entry) => entry.path)
      )
    : undefined
  const required = CONTROL_PATHS
  if (required.some((file) => !manifest.controls.some((entry) => entry.path === file)))
    throw Error('Missing HUD resolver control')
  if (strict && JSON.stringify(rootInventory(paths)) !== JSON.stringify(manifest.roots))
    throw Error('Stale HUD shared-input root inventory; regenerate shared-inputs.json')
  if (
    JSON.stringify(rootInventory(new Set(manifest.roots))) !== JSON.stringify(manifest.roots) ||
    manifest.roots.some((file) => !portable(file))
  )
    throw Error('Invalid HUD declared root inventory')
  const declaredControls = new Set(manifest.controls.map((entry) => entry.path))
  if (strict) {
    for (const file of resolverControls(
      paths,
      [...manifest.roots, ...manifest.files.map((entry) => entry.path)],
      hasGit(root) ? undefined : root
    ))
      if (!declaredControls.has(file))
        throw Error('New HUD resolver control; regenerate shared-inputs.json: ' + file)
  }
  for (const group of ['controls', 'files']) {
    if (
      manifest[group].some((entry) => !entry || typeof entry !== 'object' || Array.isArray(entry))
    )
      throw Error('Invalid HUD shared-input record')
    const names = manifest[group].map((entry) => entry.path)
    if (JSON.stringify(sorted(new Set(names))) !== JSON.stringify(names))
      throw Error('Invalid HUD shared-input list')
  }
  const sharedFiles = new Set(manifest.files.map((entry) => entry.path))
  const seen = new Set()
  for (const entry of [...manifest.controls, ...manifest.files]) {
    if (!portable(entry.path) || !/^[a-f0-9]{64}$/.test(entry.sha256) || seen.has(entry.path))
      throw Error('Invalid HUD shared-input identity')
    seen.add(entry.path)
    if (Object.keys(entry).some((key) => !['path', 'sha256', 'lfs'].includes(key)))
      throw Error('Invalid HUD shared-input record')
    if (
      Object.hasOwn(entry, 'lfs') &&
      (!entry.lfs ||
        Object.keys(entry.lfs).sort().join(',') !== 'oid,size' ||
        entry.lfs.oid !== entry.sha256 ||
        !Number.isSafeInteger(entry.lfs.size) ||
        entry.lfs.size < 0)
    )
      throw Error('Invalid HUD shared-input LFS identity')
    if (strict) {
      const actual = identity(root, entry.path, paths, entry.lfs)
      if (
        actual.sha256 !== entry.sha256 ||
        JSON.stringify(actual.lfs) !== JSON.stringify(entry.lfs)
      )
        throw Error('Stale HUD shared-input bytes: ' + entry.path)
    } else if (sharedFiles.has(entry.path)) {
      let bytes
      try {
        bytes = regular(root, entry.path)
      } catch (error) {
        if (error.code === 'ENOENT')
          throw Error('Missing shared input ' + entry.path + '; run npm run gen:shared-inputs', {
            cause: error
          })
        throw error
      }
      if (bytes.subarray(0, 43).toString().startsWith('version https://git-lfs.github.com/spec/'))
        throw Error('Unmaterialized shared input: ' + entry.path)
    }
  }
  for (const file of manifest.roots)
    if (!seen.has(file)) throw Error('Unbound HUD source root: ' + file)
  for (const edge of manifest.edges)
    if (
      !edge ||
      Object.keys(edge).sort().join(',') !== 'from,kind,to' ||
      typeof edge.kind !== 'string' ||
      !edge.kind ||
      !seen.has(edge.from) ||
      !seen.has(edge.to)
    )
      throw Error('Unbound HUD shared-input edge')
  if (manifest.files.some((entry) => !shared(entry.path)))
    throw Error('HUD contract contains an unsupported shared domain')
  return manifest.files.map((entry) => path.join(root, entry.path))
}

/** Uses the installed native compiler and Vite itself, never a second import resolver. */
export async function generateSharedInputs({ root = REPOSITORY, filteredSource = false } = {}) {
  root = realpathSync(root)
  rejectAutomaticPostcss(root)
  if (!hasGit(root) && !filteredSource)
    throw Error('Writing a shared-input contract requires a Git source checkout')
  const expectedLfs = new Map()
  let previous
  if (!hasGit(root)) {
    readSharedPaths(root)
    previous = JSON.parse(regular(root, MANIFEST).toString())
    for (const entry of [...previous.controls, ...previous.files])
      if (entry.lfs) expectedLfs.set(entry.path, entry.lfs)
  }
  const web = path.join(root, WEB),
    paths = sourceInventory(
      root,
      previous ? [...previous.controls, ...previous.files].map((entry) => entry.path) : []
    ),
    roots = rootInventory(paths)
  const controls = new Set([...roots, ...CONTROL_PATHS])
  const candidates = [...paths].filter((file) => shared(file) || file.startsWith(OWNED_SOURCE))
  const resolverCandidates = resolverControls(paths, candidates, root)
  const resolverRoles = (inventory) =>
    sorted(resolverControls(inventory, candidates, root)).map((file) => [
      file,
      inventory.has(file),
      Boolean(lstatSync(path.join(root, file), { throwIfNoEntry: false }))
    ])
  const beforeResolverRoles = resolverRoles(paths)
  const beforeInputs = new Map()
  for (const file of resolverCandidates) {
    if (!paths.has(file)) throw Error('Untracked HUD resolver control: ' + file)
    beforeInputs.set(file, sha256(regular(root, file)))
  }
  for (const file of candidates) {
    if (beforeInputs.has(file)) continue
    try {
      beforeInputs.set(file, sha256(regular(root, file)))
    } catch {
      beforeInputs.set(file, undefined)
    }
  }
  const files = new Set(),
    edges = new Map()
  const edge = (from, to, kind) => edges.set([from, to, kind].join('\0'), { from, to, kind })
  const generated = [
    WEB + '/src/engine/generated/',
    'rendering/bevy-explorer/source/deploy/web/engine/pkg/',
    'rendering/bevy-explorer/source/deploy/web/engine/pkg-scene/'
  ]
  const include = (file) => {
    // Installed packages and canonical engine producer outputs have separate input contracts.
    if (
      file.split('/').includes('node_modules') ||
      generated.some((prefix) => file.startsWith(prefix))
    )
      return false
    if (!portable(file) || (!shared(file) && !file.startsWith(OWNED_SOURCE)))
      throw Error('Unsupported resolved first-party input: ' + file)
    if (!paths.has(file))
      throw Error(
        (file.startsWith(WEB + '/')
          ? 'Untracked HUD import target: '
          : 'Untracked shared import target: ') + file
      )
    if (file.startsWith(OWNED_SOURCE)) {
      controls.add(file)
      return false
    }
    files.add(file)
    return true
  }
  const compiler = path.join(web, 'node_modules/typescript/bin/tsc')
  if (!existsSync(compiler))
    throw Error('Install the HUD locked dependencies before generating shared inputs')
  let compiled
  try {
    compiled = execFileSync(
      process.execPath,
      [
        compiler,
        '--noEmit',
        '--listFiles',
        '--pretty',
        'false',
        '-p',
        path.join(web, 'tsconfig.json')
      ],
      { cwd: web, encoding: 'utf8', maxBuffer: 1 << 28 }
    )
  } catch (error) {
    throw Error(
      'Actual HUD compiler prerequisites/typecheck failed. Run the canonical type bindings generator and HUD typecheck.\n' +
        (error.stdout?.toString() ?? error.message),
      { cause: error }
    )
  }
  for (const line of compiled.split(/\r?\n/)) {
    if (!path.isAbsolute(line.trim()) || !existsSync(line.trim())) continue
    const file = relative(root, realpathSync(line.trim()))
    if (include(file)) edge(WEB + '/tsconfig.json', file, 'type')
  }
  console.log('HUD actual compiler graph ready: ' + files.size + ' shared inputs')
  const { build, createLogger, preprocessCSS } = await import(
    pathToFileURL(path.join(web, 'node_modules/vite/dist/node/index.js')).href
  )
  const logger = createLogger('error')
  logger.warn = logger.warnOnce = (message) => {
    if (
      message.includes("didn't resolve at build time") ||
      message.includes('could not be resolved')
    )
      throw Error('Unresolved Vite dependency: ' + message)
  }
  const before = globalThis[GENERATING] ?? 0
  globalThis[GENERATING] = before + 1
  try {
    const { createVitest } = await import(
      pathToFileURL(path.join(web, 'node_modules/vitest/dist/node.js')).href
    )
    const context = await createVitest('test', {
      root: web,
      config: path.join(web, 'vitest.config.ts'),
      watch: false,
      run: true
    })
    let testEntries
    try {
      const specifications = await context.globTestSpecifications()
      testEntries = sorted(
        new Set(
          specifications.flatMap((spec) => [
            spec.moduleId,
            ...(spec.project.config.setupFiles ?? [])
          ])
        )
      )
    } finally {
      await context.close()
    }
    for (const mode of ['production', 'native', 'test']) {
      console.log('HUD Vite graph: ' + mode)
      let resolvedConfig
      const result = await build({
        root: web,
        configFile: path.join(web, mode === 'test' ? 'vitest.config.ts' : 'vite.config.ts'),
        mode,
        logLevel: 'error',
        customLogger: logger,
        plugins: [
          {
            name: 'record-hud-shared-closure',
            enforce: 'post',
            config(config) {
              if (typeof config.css?.postcss === 'string')
                throw Error(
                  'External PostCSS configuration is unsupported by the checked HUD contract'
                )
              return {
                css: {
                  postcss: {
                    plugins: [
                      {
                        postcssPlugin: 'checked-hud-css-composition',
                        Once(css) {
                          css.walkDecls((declaration) => {
                            if (
                              ['composes', 'compose-with'].includes(declaration.prop.toLowerCase())
                            )
                              throw declaration.error(
                                'Unsupported HUD CSS Modules composition: Vite/postcss-modules does not report these dependency reads. Upstream dependency reporting is required before enabling composition in this contract.'
                              )
                          })
                        }
                      }
                    ]
                  }
                }
              }
            },
            configResolved(config) {
              resolvedConfig = config
              for (const dependency of config.configFileDependencies) {
                const file = relative(root, dependency)
                include(file)
                if (portable(file) && !file.split('/').includes('node_modules')) {
                  if (!paths.has(file))
                    throw Error('Untracked Vite configuration dependency: ' + file)
                  controls.add(file)
                }
              }
              const options = config.build.rolldownOptions
              const original = options.input
              const inputs =
                mode === 'test'
                  ? Object.fromEntries(testEntries.map((file, index) => ['test' + index, file]))
                  : typeof original === 'string'
                    ? { app: original }
                    : Array.isArray(original)
                      ? Object.fromEntries(original.map((file, index) => ['entry' + index, file]))
                      : { ...original }
              let index = 0
              for (const file of sorted(files)) {
                if (/\.(?:[cm]?[jt]sx?)$/.test(file) && !/\.d\.[cm]?ts$/.test(file))
                  inputs['sharedType' + index++] = path.join(root, file)
              }
              options.input = inputs
              if (mode === 'test') {
                // SDK virtual modules and generated WASM modules have separate runtime/engine producer contracts.
                const engineOutputs = ['pkg', 'pkg-scene'].map((name) =>
                  path.join(root, 'rendering/bevy-explorer/source/deploy/web/engine', name)
                )
                options.external = (id, importer) => {
                  if (id === 'vitest' || id.startsWith('~system/') || id.startsWith('node:'))
                    return true
                  const file = path.isAbsolute(id)
                    ? id
                    : importer && id.startsWith('.')
                      ? path.resolve(path.dirname(importer), id)
                      : undefined
                  return (
                    file !== undefined &&
                    engineOutputs.some((directory) => file.startsWith(directory + path.sep))
                  )
                }
              }
            },
            async generateBundle() {
              const physical = (id) => {
                if (id.startsWith('\0')) return undefined
                const file = id.split('?')[0]
                if (!path.isAbsolute(file) || !existsSync(file)) return undefined
                return relative(root, realpathSync(file))
              }
              const cssModules = new Set()
              for (const id of this.getModuleIds()) {
                const from = physical(id),
                  info = this.getModuleInfo(id)
                if (from) include(from)
                if (from && from.endsWith('.css')) cssModules.add(from)
                if (!from || !info) continue
                for (const [kind, imports] of [
                  ['runtime', info.importedIds],
                  ['dynamic', info.dynamicallyImportedIds]
                ]) {
                  for (const imported of imports) {
                    const to = physical(imported)
                    if (!to || !include(to)) continue
                    if (
                      shared(from) ||
                      roots.includes(from) ||
                      (paths.has(from) && from.startsWith(OWNED_SOURCE))
                    ) {
                      if (!shared(from)) controls.add(from)
                      edge(from, to, mode + ':' + kind)
                    }
                  }
                }
              }
              for (const from of cssModules) {
                const result = await preprocessCSS(
                  regular(root, from).toString(),
                  path.join(root, from),
                  resolvedConfig
                )
                for (const dependency of result.deps ?? []) {
                  const to = relative(root, realpathSync(dependency))
                  if (!include(to)) continue
                  if (!shared(from)) controls.add(from)
                  edge(from, to, mode + ':css')
                }
              }
            }
          }
        ],
        build: {
          write: false,
          emptyOutDir: false,
          assetsInlineLimit: 0,
          ssr: mode === 'test',
          ssrEmitAssets: mode === 'test'
        }
      })
      // Emitting rather than inlining assets exposes Vite's resolved original filenames without resolving CSS ourselves.
      for (const output of Array.isArray(result) ? result : [result]) {
        for (const asset of output.output ?? []) {
          if (asset.type !== 'asset') continue
          for (const original of asset.originalFileNames ?? []) {
            const target = path.isAbsolute(original) ? original : path.resolve(web, original)
            if (!existsSync(target)) continue
            const file = relative(root, realpathSync(target))
            if (include(file)) edge(WEB + '/vite.config.ts', file, mode + ':asset')
          }
        }
      }
    }
  } finally {
    if (before === 0) delete globalThis[GENERATING]
    else globalThis[GENERATING] = before
  }
  // Package scopes and compiler controls alongside reachable sources are resolver inputs.
  for (const file of resolverControls(paths, [...files, ...roots])) controls.add(file)
  for (const file of controls) files.delete(file)
  const afterPaths = sourceInventory(root)
  if (JSON.stringify(rootInventory(afterPaths)) !== JSON.stringify(roots))
    throw Error('HUD source inventory changed while resolving imports')
  if (JSON.stringify(resolverRoles(afterPaths)) !== JSON.stringify(beforeResolverRoles))
    throw Error('HUD resolver control inventory changed while resolving imports')
  for (const file of new Set([...controls, ...files, ...resolverCandidates])) {
    if (beforeInputs.get(file) !== sha256(regular(root, file)))
      throw Error('HUD input changed while resolving imports: ' + file)
  }
  const manifest = {
    schema: 1,
    controls: sorted(controls).map((file) => identity(root, file, paths, expectedLfs.get(file))),
    roots,
    files: sorted(files).map((file) => identity(root, file, paths, expectedLfs.get(file))),
    edges: [...edges.values()].sort((a, b) => {
      const left = [a.from, a.to, a.kind].join('\0'),
        right = [b.from, b.to, b.kind].join('\0')
      return left < right ? -1 : left > right ? 1 : 0
    })
  }
  return Buffer.from(JSON.stringify(manifest, null, 2) + '\n')
}

if (
  process.argv[1] &&
  realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url)) &&
  !(globalThis[GENERATING] > 0)
) {
  const option = process.argv[2]
  if (process.argv.length !== 3 || !['--write', '--check'].includes(option))
    throw Error('Usage: shared-inputs.mjs --write|--check')
  // Vite imports this same module from its config; finish evaluation before awaiting the graph.
  generateSharedInputs({ filteredSource: option === '--check' })
    .then((bytes) => {
      if (option === '--write') writeFileSync(path.join(REPOSITORY, MANIFEST), bytes)
      else if (!bytes.equals(regular(REPOSITORY, MANIFEST)))
        throw Error('HUD shared-input manifest is stale; regenerate and commit it')
      console.log(
        'HUD shared-input contract ' +
          option.slice(2) +
          ': ' +
          JSON.parse(bytes).files.length +
          ' shared inputs'
      )
    })
    .catch((error) => {
      console.error(error)
      process.exitCode = 1
    })
}
