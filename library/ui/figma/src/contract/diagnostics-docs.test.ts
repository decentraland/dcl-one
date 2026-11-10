import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  DIAGNOSTICS,
  audiencesOf,
  diagnostic,
  type DiagnosticCode,
  type DiagnosticSpec
} from './diagnostics.js'

const figmaRoot = path.resolve(import.meta.dirname, '../..')
const SKILL = 'agent-kit/skills/implement-figma-design/SKILL.md'
const MAINTENANCE = 'MAINTENANCE.md'
const registry = Object.entries(DIAGNOSTICS) as [DiagnosticCode, DiagnosticSpec][]

type Row = { code: string; cells: string[] }

const MARKER = '<!-- diagnostic-codes -->'

// The rows of the one table that follows the marker comment; other tables in the document are ignored.
function codeRows(markdown: string): Row[] | undefined {
  const lines = markdown.split('\n')
  const start = lines.findIndex((line) => line.trim() === MARKER)
  if (start < 0) return undefined
  const table: string[] = []
  for (const line of lines.slice(start + 1)) {
    if (line.trim().startsWith('|')) table.push(line)
    else if (table.length) break
  }
  return table
    .map((line) => /^\s*\|\s*`([a-z][a-z-]*)`\s*\|(.*)\|\s*$/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => ({ code: match[1]!, cells: match[2]!.split('|').map((cell) => cell.trim()) }))
}

// Adds a row right below the marked table's separator, as an edit to the table would.
function withRow(markdown: string, row: string): string {
  const lines = markdown.split('\n')
  const start = lines.findIndex((line) => line.trim() === MARKER)
  const separator = lines.findIndex((line, index) => index > start && /^\s*\|\s*-/.test(line))
  lines.splice(separator + 1, 0, row)
  return lines.join('\n')
}

const read = (file: string) => readFileSync(path.join(figmaRoot, file), 'utf8')

describe('the diagnostic registry', () => {
  it('should give every code a severity, an audience and a non-empty action', () => {
    for (const [, spec] of registry) {
      expect(['error', 'warning']).toContain(spec.severity)
      expect(['consumer', 'maintainer']).toContain(spec.audience)
      const actions = typeof spec.action === 'string' ? [spec.action] : Object.values(spec.action)
      expect(actions.length).toBeGreaterThan(0)
      for (const action of actions) expect(action.trim().length).toBeGreaterThan(20)
    }
  })

  it('should attach the registry severity, audience and action to a diagnostic', () => {
    expect(diagnostic('mapping-missing', 'missing')).toEqual({
      code: 'mapping-missing',
      severity: 'error',
      audience: 'consumer',
      message: 'missing',
      action: DIAGNOSTICS['mapping-missing'].action
    })
  })

  it('should pick the action of the reason for a code with per-reason actions', () => {
    expect(diagnostic('contract-incompatible', 'range', { reason: 'library-version' }).action).toBe(
      DIAGNOSTICS['contract-incompatible'].action['library-version']
    )
  })

  it.each([[undefined], [{}], [{ reason: 'not-a-reason' }], [{ reason: 'toString' }]])(
    'should refuse a per-reason code whose details are %j',
    (details) => {
      expect(() => diagnostic('contract-incompatible', 'x', details as never)).toThrow(
        /has no action/
      )
    }
  )

  it('should refuse an unregistered reason at compile time', () => {
    // @ts-expect-error: 'library-versoin' is not a reason of contract-incompatible.
    expect(() => diagnostic('contract-incompatible', 'x', { reason: 'library-versoin' })).toThrow()
  })

  it('should give a reason that only check-mappings emits the maintainer audience', () => {
    expect(diagnostic('identity-ambiguous', 'x', { reason: 'duplicate-id' }).audience).toBe(
      'maintainer'
    )
    expect(diagnostic('identity-ambiguous', 'x', { reason: 'name' }).audience).toBe('consumer')
    expect(audiencesOf('identity-ambiguous')).toEqual(['consumer', 'maintainer'])
    expect(audiencesOf('superseded-pending')).toEqual(['maintainer'])
  })

  it('should never put a Figma URL or file key in an action', () => {
    const text = JSON.stringify(DIAGNOSTICS)
    expect(text).not.toMatch(/figma\.com/i)
    expect(text).not.toMatch(/[A-Za-z0-9]{22,}/)
  })
})

type Scope = 'consumer' | 'all'

// Differences between a document's table and the registry: missing, unknown or repeated codes, and a severity
// (or, for the maintainer table, an audience) that the registry does not give the code.
// For a per-reason code the consumer table must send the reader to the diagnostic's own `action`.
function tableProblems(markdown: string, scope: Scope): string[] {
  const rows = codeRows(markdown)
  if (!rows) return ['no diagnostic table marker']
  const expected = registry
    .filter(([code]) => scope === 'all' || audiencesOf(code).includes(scope))
    .map(([code]) => code as string)
  const listed = rows.map((row) => row.code)
  const problems = [
    ...expected.filter((code) => !listed.includes(code)).map((code) => `missing ${code}`),
    ...listed.filter((code) => !expected.includes(code)).map((code) => `unexpected ${code}`),
    ...listed.filter((code, index) => listed.indexOf(code) !== index).map((c) => `repeated ${c}`)
  ]
  for (const row of rows) {
    const spec = DIAGNOSTICS[row.code as DiagnosticCode] as DiagnosticSpec | undefined
    if (!spec) continue
    if (row.cells[0] !== spec.severity) problems.push(`${row.code} severity ${row.cells[0]}`)
    if (scope === 'all' && row.cells[1] !== audiencesOf(row.code as DiagnosticCode).join(', '))
      problems.push(`${row.code} audience ${row.cells[1]}`)
    if (
      scope === 'consumer' &&
      typeof spec.action !== 'string' &&
      !row.cells[2]?.includes('`action`')
    )
      problems.push(`${row.code} does not point at action`)
  }
  return problems
}

describe.each([
  [SKILL, 'consumer'],
  [MAINTENANCE, 'all']
] as const)('the diagnostic table of %s', (file, scope) => {
  it('should list exactly the codes its audience can receive, with their registry severity', () => {
    expect(tableProblems(read(file), scope)).toEqual([])
  })
})

describe('when a document table differs from the registry', () => {
  const skill = read(SKILL)
  const maintenance = read(MAINTENANCE)
  it.each([
    [
      'an unknown code',
      withRow(skill, '| `made-up-code` | error | x | y |'),
      'consumer',
      'unexpected made-up-code'
    ],
    [
      'a maintainer code in the consumer table',
      withRow(skill, '| `superseded-pending` | warning | x | y |'),
      'consumer',
      'unexpected superseded-pending'
    ],
    [
      'a repeated code',
      withRow(skill, '| `mapping-missing` | error | x | y |'),
      'consumer',
      'repeated mapping-missing'
    ],
    [
      'a dropped code',
      skill.replace(/^\| `identity-uncertain`.*$/m, ''),
      'consumer',
      'missing identity-uncertain'
    ],
    [
      'a wrong severity',
      skill.replace(/^(\| `figma-drift`\s*\|)\s*warning/m, '$1 error'),
      'consumer',
      'figma-drift severity error'
    ],
    [
      'a per-reason code whose row does not point at action',
      skill.replace(/^(\| `identity-ambiguous`.*\|)[^|]*\|\s*$/m, '$1 Call again with a nodeId |'),
      'consumer',
      'identity-ambiguous does not point at action'
    ],
    [
      'a wrong audience',
      maintenance.replace(
        /^(\| `superseded-by-sync`\s*\|\s*warning\s*\|)\s*maintainer/m,
        '$1 consumer'
      ),
      'all',
      'superseded-by-sync audience consumer'
    ],
    [
      'a mixed code given one audience',
      maintenance.replace(/^(\| `identity-ambiguous`\s*\|\s*error\s*\|)[^|]*/m, '$1 consumer '),
      'all',
      'identity-ambiguous audience consumer'
    ],
    ['no marker', skill.replace(MARKER, ''), 'consumer', 'no diagnostic table marker']
  ] as const)('should report %s', (_label, markdown, scope, problem) => {
    expect(tableProblems(markdown, scope)).toContain(problem)
  })

  it('should ignore another table keyed by backticked names', () => {
    expect(
      tableProblems(
        skill + '\n\n| Command | x |\n| --- | --- |\n| `fetch-source` | y |\n',
        'consumer'
      )
    ).toEqual([])
  })
})
