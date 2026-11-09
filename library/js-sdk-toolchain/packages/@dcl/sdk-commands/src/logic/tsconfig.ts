import path from 'path'
import i18next from 'i18next'
import {
  createScanner,
  findNodeAtLocation,
  getNodeValue,
  parseTree,
  SyntaxKind,
  type JSONPath,
  type Node,
  type ParseError
} from 'jsonc-parser'
import { CliComponents } from '../components'
import { printWarning } from './beautiful-logs'

type TsconfigComponents = Pick<CliComponents, 'logger' | 'fs'>

type ConfigFile = {
  file: string
  text: string
  root: Node
  /** this file's own `compilerOptions.baseUrl`, absolute */
  baseUrl?: string
  /** this file's own `compilerOptions.paths` */
  paths?: Record<string, unknown>
  /** this file's own `compilerOptions.strict` */
  strict?: boolean
  /** an installed package's file, or one outside the project: read, never rewritten */
  external: boolean
  /** the files this one extends, in `extends` order; a target that could not be read stays a name */
  parents: Parent[]
}

type Parent = ConfigFile | { unresolved: string }

const isConfig = (parent: Parent): parent is ConfigFile => 'file' in parent

/** files read per project: a chain longer than this is left alone */
const MAX_CHAIN = 16

/**
 * TypeScript 7 removed `baseUrl`, and `paths` entries have to start with `./` or `../`: they
 * resolve against the file that declares them. Scenes written for earlier releases carry
 * `"baseUrl": "."` with bare `paths` (the scene templates do), so before the type checker runs
 * the scene's own tsconfig files are rewritten: `baseUrl` is deleted, every `paths` entry is
 * re-rooted at its file, and a `"*"` entry keeps bare imports that resolved through `baseUrl`
 * working. TypeScript takes `paths` from the nearest file that declares them, so the `"*"` entry
 * goes into every file whose `paths` are in force under a `baseUrl`, and a file that declares a
 * `baseUrl` but no `paths` receives the `paths` it inherited along with the `"*"` entry. Files
 * reached through a relative `extends` inside the project are rewritten too; files of installed
 * packages are not. Each rewrite is reported as a warning. The rewrite never fails a build: a
 * file that cannot be read, parsed or written is left to the type checker.
 *
 * TypeScript 7 also turns `strict` on by default. A scene whose config chain (the installed
 * packages it extends included) never sets `strict` was type-checked without it so far, so its
 * tsconfig gets an explicit `"strict": false` and a warning, and the scene keeps building. When
 * an `extends` target cannot be resolved the setting in force is unknown: the file is left alone
 * and a warning says so.
 *
 * The Creator Hub's scene template maps `@dcl/asset-packs/*` under `paths` to the copy nested in
 * `@dcl/inspector`, which the SDK no longer installs: `@dcl/smart-items` replaces that package and
 * the bundler resolves its imports there, so `@dcl/asset-packs` entries are pointed at it (a bare
 * `@dcl/asset-packs` entry is added beside a wildcard one) and a warning says so.
 *
 * Returns the files that were rewritten.
 */
export async function fixTsconfigForTypescript7(
  components: TsconfigComponents,
  tsconfig: string,
  projectDir: string
): Promise<string[]> {
  const rewritten: string[] = []
  const chain = await readConfigChain(components, tsconfig, projectDir)
  const strict = chain.length ? effectiveStrict(chain[0]) : undefined
  if (typeof strict === 'object') {
    printWarning(
      components.logger,
      i18next.t('warnings.bundle.tsconfig_strict_unresolved', { tsconfig, target: strict.unresolved })
    )
  }
  const smartItems = await smartItemsDir(components, projectDir)
  for (const config of chain) {
    if (config.external) continue
    const result = rewriteConfig(config, config === chain[0] && strict === undefined, smartItems)
    if (!result) continue
    try {
      await components.fs.writeFile(config.file, result.text)
    } catch (error: any) {
      const params = { tsconfig: config.file, error: error.message }
      if (result.pathsChanged) {
        printWarning(components.logger, i18next.t('warnings.bundle.tsconfig_not_writable', params))
      }
      if (result.strictKept) {
        printWarning(components.logger, i18next.t('warnings.bundle.tsconfig_strict_not_writable', params))
      }
      continue
    }
    rewritten.push(config.file)
    if (result.pathsChanged) {
      printWarning(components.logger, i18next.t('warnings.bundle.tsconfig_base_url_removed', { tsconfig: config.file }))
    }
    if (result.starPath) {
      printWarning(
        components.logger,
        i18next.t('warnings.bundle.tsconfig_star_path_added', { tsconfig: config.file, target: result.starPath })
      )
    }
    if (result.strictKept) {
      printWarning(
        components.logger,
        i18next.t('warnings.bundle.tsconfig_strict_default_kept', { tsconfig: config.file })
      )
    }
    if (result.smartItemsPath) {
      printWarning(
        components.logger,
        i18next.t('warnings.bundle.tsconfig_asset_packs_paths', {
          tsconfig: config.file,
          target: result.smartItemsPath
        })
      )
    }
  }
  return rewritten
}

/**
 * The `@dcl/smart-items` package a build of `projectDir` bundles, as the project reaches it: the copy
 * under its own `node_modules` (a path the tsconfig can carry across machines), else the nearest copy
 * up the tree, else the one installed beside this package.
 */
async function smartItemsDir(components: TsconfigComponents, projectDir: string): Promise<string | undefined> {
  for (let dir = path.resolve(projectDir); ; dir = path.dirname(dir)) {
    const candidate = path.join(dir, 'node_modules', '@dcl', 'smart-items')
    if (await components.fs.directoryExists(candidate)) return candidate
    if (path.dirname(dir) === dir) break
  }
  try {
    return path.dirname(require.resolve('@dcl/smart-items/package.json', { paths: [__dirname] }))
  } catch {
    return undefined
  }
}

/** the `@dcl/smart-items` target of an `@dcl/asset-packs` paths key: `dist/<module>` lives at the same place */
function smartItemsTarget(key: string, smartItems: string): string | undefined {
  const match = /^@dcl\/asset-packs(?:\/(.*))?$/.exec(key)
  if (!match) return undefined
  return match[1] === undefined ? `${smartItems}/dist/index` : `${smartItems}/${match[1]}`
}

async function readConfigChain(
  components: TsconfigComponents,
  tsconfig: string,
  projectDir: string
): Promise<ConfigFile[]> {
  const known = new Map<string, ConfigFile>()
  const visiting = new Set<string>()
  const chain: ConfigFile[] = []

  async function visit(file: string, external: boolean): Promise<ConfigFile | undefined> {
    const seen = known.get(file)
    if (seen) return visiting.has(file) ? undefined : seen
    if (chain.length >= MAX_CHAIN) return
    const text = await readText(components, file)
    if (text === undefined) return
    const errors: ParseError[] = []
    const root = parseTree(text, errors, { allowTrailingComma: true })
    if (!root || root.type !== 'object' || errors.length) return
    const dir = path.dirname(file)
    const baseUrl = stringAt(root, ['compilerOptions', 'baseUrl'])
    const paths = findNodeAtLocation(root, ['compilerOptions', 'paths'])
    const config: ConfigFile = {
      file,
      text,
      root,
      baseUrl: baseUrl === undefined ? undefined : path.resolve(dir, baseUrl),
      paths: paths?.type === 'object' ? getNodeValue(paths) : undefined,
      strict: booleanAt(root, ['compilerOptions', 'strict']),
      external,
      parents: []
    }
    known.set(file, config)
    chain.push(config)
    visiting.add(file)
    for (const target of extendsTargets(root)) {
      const parentFile = typeof target === 'string' ? await resolveExtends(components, dir, target) : undefined
      const parent = parentFile && (await visit(parentFile, external || !insideProject(parentFile, projectDir)))
      config.parents.push(parent || { unresolved: String(target) })
    }
    visiting.delete(file)
    return config
  }

  await visit(tsconfig, false)
  return chain
}

async function readText(components: TsconfigComponents, file: string): Promise<string | undefined> {
  try {
    return await components.fs.readFile(file, 'utf-8')
  } catch {
    return undefined
  }
}

async function readJson(components: TsconfigComponents, file: string): Promise<any> {
  const text = await readText(components, file)
  try {
    return text === undefined ? undefined : JSON.parse(text)
  } catch {
    return undefined
  }
}

async function isFile(components: TsconfigComponents, file: string): Promise<boolean> {
  try {
    return (await components.fs.stat(file)).isFile()
  } catch {
    return false
  }
}

async function isDirectory(components: TsconfigComponents, dir: string): Promise<boolean> {
  try {
    return (await components.fs.stat(dir)).isDirectory()
  } catch {
    return false
  }
}

function stringAt(root: Node, jsonPath: JSONPath): string | undefined {
  const node = findNodeAtLocation(root, jsonPath)
  return node?.type === 'string' ? (getNodeValue(node) as string) : undefined
}

function booleanAt(root: Node, jsonPath: JSONPath): boolean | undefined {
  const node = findNodeAtLocation(root, jsonPath)
  return node?.type === 'boolean' ? (getNodeValue(node) as boolean) : undefined
}

/** the `extends` targets of a config: relative or absolute paths, or package names; anything else is kept to be reported */
function extendsTargets(root: Node): unknown[] {
  const node = findNodeAtLocation(root, ['extends'])
  if (!node) return []
  const value: unknown = getNodeValue(node)
  return Array.isArray(value) ? value : [value]
}

/**
 * `extends` resolution as TypeScript does it. A relative or absolute target is the file, or the
 * file with `.json` appended. A package name is looked up in the `node_modules` of the file's
 * directory and of its ancestors: through the package's `exports` map when it has one, else the
 * file the target names (with `.json` appended when missing), else for a directory the file its
 * package.json `tsconfig` field names, else its `tsconfig.json`.
 */
async function resolveExtends(
  components: TsconfigComponents,
  dir: string,
  target: string
): Promise<string | undefined> {
  if (target.startsWith('./') || target.startsWith('../') || path.isAbsolute(target)) {
    return resolveFileOrDirectory(components, path.resolve(dir, target), false)
  }
  const [, packageName, subpath] = /^((?:@[^/]+\/)?[^/]+)(?:\/(.*))?$/.exec(target) ?? []
  if (!packageName) return undefined
  for (let current = dir; ; current = path.dirname(current)) {
    const packageDir = path.join(current, 'node_modules', packageName)
    if (await isDirectory(components, packageDir)) {
      const manifest = await readJson(components, path.join(packageDir, 'package.json'))
      const resolved =
        manifest?.exports !== undefined
          ? await firstFile(
              components,
              exportTargets(manifest.exports, subpath === undefined ? '.' : `./${subpath}`).map((file) =>
                path.resolve(packageDir, file)
              )
            )
          : await resolveFileOrDirectory(components, path.join(packageDir, subpath ?? ''), true)
      if (resolved) return resolved
    }
    if (path.dirname(current) === current) return undefined
  }
}

async function resolveFileOrDirectory(
  components: TsconfigComponents,
  candidate: string,
  inPackage: boolean
): Promise<string | undefined> {
  const file = await firstFile(components, candidate.endsWith('.json') ? [candidate] : [candidate, `${candidate}.json`])
  if (file || !inPackage || !(await isDirectory(components, candidate))) return file
  const manifest = await readJson(components, path.join(candidate, 'package.json'))
  const named = typeof manifest?.tsconfig === 'string' ? path.resolve(candidate, manifest.tsconfig) : undefined
  return firstFile(components, named ? [named] : [path.join(candidate, 'tsconfig.json')])
}

async function firstFile(components: TsconfigComponents, candidates: string[]): Promise<string | undefined> {
  for (const candidate of candidates) {
    if (await isFile(components, candidate)) return candidate
  }
  return undefined
}

/** the conditions TypeScript applies to a config lookup through an `exports` map */
const EXPORT_CONDITIONS = new Set(['require', 'types', 'node', 'default'])

/** the files an `exports` map names for a subpath, in the order they are to be tried */
function exportTargets(exports: unknown, subpath: string): string[] {
  const isMap =
    exports !== null &&
    typeof exports === 'object' &&
    !Array.isArray(exports) &&
    Object.keys(exports)[0]?.startsWith('.')
  const map: Record<string, unknown> = isMap ? (exports as Record<string, unknown>) : { '.': exports }
  if (subpath in map) return exportFiles(map[subpath], '')
  let best: { prefix: string; match: string; target: unknown } | undefined
  for (const [key, target] of Object.entries(map)) {
    const star = key.indexOf('*')
    if (star < 0 || key.includes('*', star + 1)) continue
    const prefix = key.slice(0, star)
    const suffix = key.slice(star + 1)
    const matches =
      subpath.length >= prefix.length + suffix.length && subpath.startsWith(prefix) && subpath.endsWith(suffix)
    if (matches && (!best || prefix.length > best.prefix.length)) {
      best = { prefix, match: subpath.slice(prefix.length, subpath.length - suffix.length), target }
    }
  }
  return best ? exportFiles(best.target, best.match) : []
}

function exportFiles(target: unknown, match: string): string[] {
  if (typeof target === 'string') return target.startsWith('./') ? [target.split('*').join(match)] : []
  if (Array.isArray(target)) return target.flatMap((entry) => exportFiles(entry, match))
  if (target && typeof target === 'object') {
    for (const [condition, value] of Object.entries(target)) {
      if (!EXPORT_CONDITIONS.has(condition)) continue
      const files = exportFiles(value, match)
      if (files.length) return files
    }
  }
  return []
}

function insideProject(file: string, projectDir: string): boolean {
  const relative = path.relative(projectDir, file)
  const segments = relative.split(path.sep)
  return relative !== '' && segments[0] !== '..' && !path.isAbsolute(relative) && !segments.includes('node_modules')
}

/** a setting in force for a file: its own, else the nearest one along `extends`, where a later target overrides an earlier one */
function inherited<T>(
  config: ConfigFile,
  own: (config: ConfigFile) => T | undefined,
  projectOnly: boolean
): T | undefined {
  const value = own(config)
  if (value !== undefined) return value
  for (const parent of [...config.parents].reverse()) {
    if (!isConfig(parent) || (projectOnly && parent.external)) continue
    const found = inherited(parent, own, projectOnly)
    if (found !== undefined) return found
  }
  return undefined
}

/** the `baseUrl` in force for a file, among the project files */
const effectiveBaseUrl = (config: ConfigFile) => inherited(config, (file) => file.baseUrl, true)

/** the `paths` in force for a file: TypeScript takes the nearest declaration as a whole */
const effectivePaths = (config: ConfigFile) => inherited(config, (file) => file.paths, false)

/** the `strict` in force for a file; unknown when an unresolved `extends` target comes before the nearest setting */
function effectiveStrict(config: ConfigFile): boolean | { unresolved: string } | undefined {
  if (config.strict !== undefined) return config.strict
  for (const parent of [...config.parents].reverse()) {
    const found = isConfig(parent) ? effectiveStrict(parent) : parent
    if (found !== undefined) return found
  }
  return undefined
}

/** `./`-rooted path from the directory of a config file to a target */
function relativeToFile(dir: string, target: string): string {
  const relative = path.relative(dir, target).split(path.sep).join('/')
  if (relative === '') return '.'
  return relative.startsWith('../') || relative === '..' ? relative : `./${relative}`
}

type Rewrite = {
  text: string
  pathsChanged: boolean
  starPath?: string
  strictKept: boolean
  smartItemsPath?: string
}

function rewriteConfig(config: ConfigFile, keepStrictDefault: boolean, smartItems?: string): Rewrite | undefined {
  const dir = path.dirname(config.file)
  const editor = new ConfigEditor(config.text)

  const baseUrl = effectiveBaseUrl(config)
  const paths = findNodeAtLocation(config.root, ['compilerOptions', 'paths'])
  let starPath: string | undefined
  if (baseUrl) {
    const star = `${relativeToFile(dir, baseUrl)}/*`
    const reroot = (entries: unknown[]) =>
      entries.map((entry) =>
        typeof entry === 'string' && !path.isAbsolute(entry) ? relativeToFile(dir, path.resolve(baseUrl, entry)) : entry
      )
    if (paths?.type === 'object') {
      for (const property of paths.children!) {
        const [key, value] = property.children as [Node, Node]
        if (value.type !== 'array') continue
        const entries: unknown[] = getNodeValue(value)
        const rerooted = reroot(entries)
        if (rerooted.some((entry, index) => entry !== entries[index])) {
          editor.replace(['compilerOptions', 'paths', key.value], rerooted)
        }
      }
      if (!findNodeAtLocation(paths, ['*'])) {
        starPath = star
        editor.insert(['compilerOptions', 'paths'], '*', oneLine([star]))
      }
    } else if (!paths && config.baseUrl) {
      const merged = Object.entries(effectivePaths(config) ?? {}).map(([key, entries]) => [
        key,
        Array.isArray(entries) ? reroot(entries) : entries
      ])
      if (!merged.some(([key]) => key === '*')) {
        starPath = star
        merged.push(['*', [star]])
      }
      editor.insert(['compilerOptions'], 'paths', oneLineObject(merged))
    }
    if (config.baseUrl) editor.remove(['compilerOptions', 'baseUrl'])
  }
  const pathsChanged = editor.changed

  let smartItemsPath: string | undefined
  if (smartItems && paths?.type === 'object') {
    const relative = relativeToFile(dir, smartItems)
    const keys = paths.children!.map((property) => property.children![0].value as string)
    for (const key of keys) {
      const target = smartItemsTarget(key, relative)
      const value = findNodeAtLocation(paths, [key])
      if (!target || value?.type !== 'array') continue
      const entries: unknown[] = getNodeValue(value)
      if (entries.length === 1 && entries[0] === target) continue
      editor.replace(['compilerOptions', 'paths', key], [target])
      smartItemsPath = relative
    }
    if (smartItemsPath && !keys.includes('@dcl/asset-packs')) {
      editor.insert(['compilerOptions', 'paths'], '@dcl/asset-packs', oneLine([`${relative}/dist/index`]))
    }
  }

  let strictKept = false
  if (keepStrictDefault) {
    const compilerOptions = findNodeAtLocation(config.root, ['compilerOptions'])
    if (!compilerOptions) {
      editor.insert([], 'compilerOptions', '{ "strict": false }')
      strictKept = true
    } else if (compilerOptions.type === 'object') {
      editor.insert(['compilerOptions'], 'strict', 'false')
      strictKept = true
    }
  }

  return editor.changed ? { text: editor.text, pathsChanged, starPath, strictKept, smartItemsPath } : undefined
}

function oneLine(values: unknown[]): string {
  return `[${values.map((value) => JSON.stringify(value)).join(', ')}]`
}

function oneLineObject(entries: unknown[][]): string {
  const properties = entries.map(
    ([key, value]) => `${JSON.stringify(key)}: ${Array.isArray(value) ? oneLine(value) : JSON.stringify(value)}`
  )
  return `{ ${properties.join(', ')} }`
}

type Token = { kind: SyntaxKind; offset: number; end: number }

/**
 * Edits a JSONC document in place, keeping its comments and layout: jsonc-parser's `modify` reformats
 * the line it edits, which turns a sibling's one-line array into one element per line.
 */
class ConfigEditor {
  changed = false
  private readonly eol: string

  constructor(public text: string) {
    this.eol = text.includes('\r\n') ? '\r\n' : '\n'
  }

  /** replaces an array, written on one line */
  replace(jsonPath: JSONPath, value: unknown[]) {
    const node = findNodeAtLocation(this.root(), jsonPath)!
    this.splice(node.offset, node.length, oneLine(value))
  }

  /**
   * adds a property at the end of an object: on a line of its own when the object spans lines, below
   * a comment that trails the last property
   */
  insert(objectPath: JSONPath, key: string, valueText: string) {
    const object = findNodeAtLocation(this.root(), objectPath)!
    const property = `${JSON.stringify(key)}: ${valueText}`
    const last = object.children![object.children!.length - 1]
    if (!last) {
      this.splice(object.offset + 1, object.length - 2, ` ${property} `)
      return
    }
    const lastEnd = last.offset + last.length
    if (!this.text.slice(object.offset, last.offset).includes('\n')) {
      this.splice(lastEnd, 0, `, ${property}`)
      return
    }
    const tokens = this.tokens()
    let lineEnd = lastEnd
    let comma = false
    for (let index = tokens.findIndex((token) => token.offset >= lastEnd); index < tokens.length; index++) {
      const token = tokens[index]
      if (token.kind === SyntaxKind.Trivia) continue
      const sameLine =
        token.kind !== SyntaxKind.LineBreakTrivia && !this.text.slice(token.offset, token.end).includes('\n')
      if (token.kind === SyntaxKind.CommaToken) comma = true
      else if (
        !sameLine ||
        (token.kind !== SyntaxKind.LineCommentTrivia && token.kind !== SyntaxKind.BlockCommentTrivia)
      )
        break
      lineEnd = token.end
    }
    this.splice(lineEnd, 0, `${this.eol}${this.lineIndent(last.offset)}${property}`)
    if (!comma) this.splice(lastEnd, 0, ',')
  }

  /**
   * removes a property with the comma that separates it from its neighbours, and with its line when
   * nothing else is on it; comments are never removed, so one trailing the property keeps its line
   */
  remove(jsonPath: JSONPath) {
    const property = findNodeAtLocation(this.root(), jsonPath)!.parent!
    const siblings = property.parent!.children!
    const previous = siblings[siblings.indexOf(property) - 1] as Node | undefined
    const tokens = this.tokens()
    const propertyEnd = property.offset + property.length
    const next = this.significantAfter(tokens, propertyEnd)
    const comma = next?.kind === SyntaxKind.CommaToken ? next : undefined
    const start = property.offset
    let end = comma ? comma.end : propertyEnd
    let index = tokens.findIndex((token) => token.offset >= end)
    while (tokens[index]?.kind === SyntaxKind.Trivia) end = tokens[index++].end
    const following = tokens[index] as Token | undefined
    const atLineEnd = !following || following.kind === SyntaxKind.LineBreakTrivia
    const lineStart = this.text.lastIndexOf('\n', start - 1) + 1
    const leading = /[ \t]*$/.exec(this.text.slice(lineStart, start))![0].length
    const ownsLine = leading === start - lineStart
    if (ownsLine && atLineEnd) {
      this.splice(lineStart, (following?.end ?? end) - lineStart, '')
    } else if (atLineEnd) {
      this.splice(start - leading, end - start + leading, '')
    } else if (ownsLine || comma) {
      this.splice(start, end - start, '')
    } else {
      this.splice(start - leading, propertyEnd - start + leading, '')
    }
    if (!comma && previous) {
      const separator = this.significantAfter(tokens, previous.offset + previous.length)!
      this.splice(separator.offset, separator.end - separator.offset, '')
    }
  }

  private root(): Node {
    return parseTree(this.text, undefined, { allowTrailingComma: true })!
  }

  private tokens(): Token[] {
    const scanner = createScanner(this.text, false)
    const tokens: Token[] = []
    for (let kind = scanner.scan(); kind !== SyntaxKind.EOF; kind = scanner.scan()) {
      const offset = scanner.getTokenOffset()
      tokens.push({ kind, offset, end: offset + scanner.getTokenLength() })
    }
    return tokens
  }

  /** the first token at or after an offset that is not whitespace, a line break or a comment */
  private significantAfter(tokens: Token[], offset: number): Token | undefined {
    return tokens.find(
      (token) =>
        token.offset >= offset &&
        token.kind !== SyntaxKind.Trivia &&
        token.kind !== SyntaxKind.LineBreakTrivia &&
        token.kind !== SyntaxKind.LineCommentTrivia &&
        token.kind !== SyntaxKind.BlockCommentTrivia
    )
  }

  private lineIndent(offset: number): string {
    const lineStart = this.text.lastIndexOf('\n', offset - 1) + 1
    return /^[ \t]*/.exec(this.text.slice(lineStart, offset))![0]
  }

  private splice(offset: number, length: number, content: string) {
    this.text = this.text.slice(0, offset) + content + this.text.slice(offset + length)
    this.changed = true
  }
}
