import ts from 'typescript'
import { readFile } from 'node:fs/promises'
import { diagnostic, type Diagnostic } from './diagnostics.js'

// The subset of a Code Connect template (.figma.ts) this MCP reads statically. Templates are never
// executed here: anything outside this subset is reported instead of guessed.
const SUPPORTED_CONTRACT = 1

type EnumLocal = {
  kind: 'enum'
  local: string
  figmaProperty: string
  values: Record<string, string>
}
type BooleanLocal = {
  kind: 'boolean'
  local: string
  figmaProperty: string
  whenTrue: string
  whenFalse: string
}
type StringLocal = { kind: 'string'; local: string; figmaProperty: string }
type TextLocal = { kind: 'text'; local: string; layer: string }
type MappingLocal = EnumLocal | BooleanLocal | StringLocal | TextLocal

type ParsedMapping = {
  file: string
  header: {
    url?: string
    source?: string
    component?: string
    contract?: string
    states: string[]
  }
  figma?: { fileKey: string; nodeId: string }
  locals: MappingLocal[]
  example?: string
  imports: string[]
  id?: string
  issues: Diagnostic[]
}

const STRING = String.raw`(?:'([^']*)'|"([^"]*)")`
const KEY = String.raw`(?:'([^']*)'|"([^"]*)"|([A-Za-z_][\w-]*))`

function pick(...groups: (string | undefined)[]): string | undefined {
  return groups.find((group) => group !== undefined)
}

function parseHeader(source: string): ParsedMapping['header'] {
  const header: ParsedMapping['header'] = { states: [] }
  for (const match of source.matchAll(/^\/\/\s*(url|source|component|contract|states)=(.*)$/gm)) {
    const [, key, value] = match
    const trimmed = value!.trim()
    if (key === 'states') header.states = trimmed.split(',').map((s) => s.trim())
    else header[key as 'url' | 'source' | 'component' | 'contract'] = trimmed
  }
  return header
}

// Mappings never carry the private Figma file key. Their url names the file through this
// placeholder, which Code Connect resolves at publish time only, outside this tree's code
// (documentUrlSubstitutions).
const FIGMA_FILE_PLACEHOLDER = '<DCL_FIGMA_FILE>'
const LITERAL_FIGMA_URL = 'literal-figma-url'

function parseFigmaUrl(url: string): { fileKey: string; nodeId: string } | undefined {
  const placeholder = /^<DCL_FIGMA_FILE>(?:\/[^?]*)?\?(?:[^#]*&)?node-id=(\d+)[-:](\d+)/.exec(url)
  if (placeholder)
    return { fileKey: FIGMA_FILE_PLACEHOLDER, nodeId: `${placeholder[1]}:${placeholder[2]}` }
  // A literal Figma URL is parsed only so validation can reject it; its key is not kept.
  const literal =
    /figma\.com\/(?:design|file)\/[0-9a-zA-Z]+\/[^?]*\?(?:[^#]*&)?node-id=(\d+)[-:](\d+)/.exec(url)
  return literal ? { fileKey: LITERAL_FIGMA_URL, nodeId: `${literal[1]}:${literal[2]}` } : undefined
}

function parseEntries(body: string): Record<string, string> | undefined {
  const entries: Record<string, string> = {}
  const entry = new RegExp(`${KEY}\\s*:\\s*${STRING}`, 'g')
  let consumed = body.replace(entry, (...m) => {
    const key = pick(m[1], m[2], m[3])
    const value = pick(m[4], m[5])
    if (key !== undefined && value !== undefined) entries[key] = value
    return ''
  })
  consumed = consumed.replace(/[\s,]/g, '')
  return consumed === '' ? entries : undefined
}

function parseLocals(source: string, file: string, issues: Diagnostic[]): MappingLocal[] {
  const locals: MappingLocal[] = []
  const claimed = new Set<number>()

  const claim = (match: RegExpMatchArray) => {
    claimed.add(match.index!)
    return match
  }

  for (const m of source.matchAll(
    new RegExp(String.raw`const (\w+) = instance\.getEnum\(${STRING},\s*\{([\s\S]*?)\}\s*\)`, 'g')
  )) {
    claim(m)
    const values = parseEntries(m[4]!)
    if (!values) {
      issues.push(
        diagnostic(
          'unsupported-construct',
          'error',
          `${file}: getEnum values must be string literals`,
          {
            local: m[1]
          }
        )
      )
      continue
    }
    locals.push({ kind: 'enum', local: m[1]!, figmaProperty: pick(m[2], m[3])!, values })
  }

  for (const m of source.matchAll(
    new RegExp(
      String.raw`const (\w+) = instance\.getBoolean\(${STRING},\s*\{([\s\S]*?)\}\s*\)`,
      'g'
    )
  )) {
    claim(m)
    const values = parseEntries(m[4]!)
    if (!values || values.true === undefined || values.false === undefined) {
      issues.push(
        diagnostic(
          'unsupported-construct',
          'error',
          `${file}: getBoolean needs a { true, false } map of string literals`,
          { local: m[1] }
        )
      )
      continue
    }
    locals.push({
      kind: 'boolean',
      local: m[1]!,
      figmaProperty: pick(m[2], m[3])!,
      whenTrue: values.true,
      whenFalse: values.false
    })
  }

  for (const m of source.matchAll(
    new RegExp(String.raw`const (\w+) = instance\.getString\(${STRING}\)`, 'g')
  )) {
    claim(m)
    locals.push({ kind: 'string', local: m[1]!, figmaProperty: pick(m[2], m[3])! })
  }

  const layers = new Map<string, string>()
  for (const m of source.matchAll(
    new RegExp(String.raw`const (\w+) = instance\.findText\(${STRING}\)`, 'g')
  )) {
    claim(m)
    layers.set(m[1]!, pick(m[2], m[3])!)
  }
  for (const m of source.matchAll(
    /const (\w+) = (\w+)\.type === 'TEXT' \? \2\.textContent : ''/g
  )) {
    const layer = layers.get(m[2]!)
    if (layer !== undefined) locals.push({ kind: 'text', local: m[1]!, layer })
  }

  const calls = [...source.matchAll(/instance\.(\w+)\(/g)].map((call) => call[1]!)
  const unsupported = calls.filter(
    (name) => !['getEnum', 'getBoolean', 'getString', 'findText'].includes(name)
  )
  if (unsupported.length > 0 || calls.length > claimed.size) {
    issues.push(
      diagnostic(
        'unsupported-construct',
        'error',
        `${file}: template uses constructs this reader does not support${unsupported.length ? ` (${[...new Set(unsupported)].join(', ')})` : ' (a call outside a plain const declaration)'}`
      )
    )
  }
  if (/figma\.(?!selectedInstance|code\b|tsx\b|html\b)\w+/.test(source)) {
    issues.push(
      diagnostic(
        'unsupported-construct',
        'error',
        `${file}: only figma.selectedInstance and figma.tsx/code/html are supported`
      )
    )
  }
  return locals
}

function parseMappingSource(raw: string, file: string): ParsedMapping {
  const issues: Diagnostic[] = []
  const header = parseHeader(raw)
  // Header lines are metadata, not template code; leaving them in would trip the construct checks.
  const source = raw.replace(/^\/\/.*$/gm, '')
  const figma = header.url ? parseFigmaUrl(header.url) : undefined
  if (!figma) {
    issues.push(
      diagnostic(
        'unsupported-construct',
        'error',
        `${file}: missing or unreadable "// url=" Figma link with a node-id`
      )
    )
  }

  const example = /example:\s*figma\.(?:tsx|code|html)`([\s\S]*?)`/.exec(source)?.[1]
  const importsBlock = /imports:\s*\[([\s\S]*?)\]/.exec(source)?.[1]
  const imports = importsBlock
    ? [...importsBlock.matchAll(new RegExp(STRING, 'g'))].map((m) => pick(m[1], m[2])!)
    : []
  const id = /\bid:\s*'([^']+)'/.exec(source)?.[1]
  if (example === undefined) {
    issues.push(
      diagnostic('unsupported-construct', 'error', `${file}: no figma.tsx/code example found`)
    )
  }

  const locals = parseLocals(source, file, issues)
  const substitutions = [...(example ?? '').matchAll(/\$\{([^}]+)\}/g)]
  const template = (example ?? '').replace(/\$\{[^}]+\}/g, (match) => 'X'.repeat(match.length))
  const syntax = ts.createSourceFile(
    'usage.tsx',
    template,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX
  )
  for (const match of substitutions) {
    const local = locals.find((item) => item.local === match[1]!.trim())
    if (!local || (local.kind !== 'text' && local.kind !== 'string')) continue
    const owner = (node: ts.Node): ts.Node | undefined => {
      if (node.getStart(syntax) > match.index || node.end < match.index + match[0].length)
        return undefined
      return ts.forEachChild(node, owner) ?? node
    }
    const node = owner(syntax)
    if (
      !node ||
      !(ts.isJsxText(node) || (ts.isStringLiteral(node) && ts.isJsxAttribute(node.parent)))
    )
      issues.push(
        diagnostic(
          'unsupported-construct',
          'error',
          `${file}: text placeholders are supported only in JSX text or quoted attributes, not JavaScript expressions`
        )
      )
  }
  return {
    file,
    header,
    figma,
    locals,
    example,
    imports,
    id,
    issues
  }
}

async function readMappingFile(file: string): Promise<ParsedMapping> {
  return parseMappingSource(await readFile(file, 'utf8'), file)
}

export {
  FIGMA_FILE_PLACEHOLDER,
  SUPPORTED_CONTRACT,
  parseFigmaUrl,
  parseMappingSource,
  readMappingFile
}
export type { MappingLocal, ParsedMapping }
