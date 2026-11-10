import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { DIAGNOSTICS, diagnostic, type DiagnosticCode, type DiagnosticSpec } from './diagnostics.js'

const figmaRoot = path.resolve(import.meta.dirname, '../..')
const SKILL = 'agent-kit/skills/implement-figma-design/SKILL.md'
const MAINTENANCE = 'MAINTENANCE.md'
const registry = Object.entries(DIAGNOSTICS) as [DiagnosticCode, DiagnosticSpec][]

type Row = { code: string; cells: string[] }

// Every Markdown table row whose first cell is a backticked code, wherever it sits in the document.
function codeRows(markdown: string): Row[] {
  return markdown
    .split('\n')
    .map((line) => /^\|\s*`([a-z][a-z-]*)`\s*\|(.*)\|\s*$/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => ({ code: match[1]!, cells: match[2]!.split('|').map((cell) => cell.trim()) }))
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
      expect(() => diagnostic('contract-incompatible', 'x', details)).toThrow(/has no action/)
    }
  )

  it('should never put a Figma URL or file key in an action', () => {
    const text = JSON.stringify(DIAGNOSTICS)
    expect(text).not.toMatch(/figma\.com/i)
    expect(text).not.toMatch(/[A-Za-z0-9]{22,}/)
  })
})

type Scope = 'consumer' | 'all'

// Differences between a document's table and the registry: missing, unknown or repeated codes, and a severity
// (or, for the maintainer table, an audience) that the registry does not give the code.
function tableProblems(markdown: string, scope: Scope): string[] {
  const rows = codeRows(markdown)
  const expected = registry
    .filter(([, spec]) => scope === 'all' || spec.audience === scope)
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
    if (scope === 'all' && row.cells[1] !== spec.audience)
      problems.push(`${row.code} audience ${row.cells[1]}`)
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
      skill + '\n| `made-up-code` | error | x | y |\n',
      'consumer',
      'unexpected made-up-code'
    ],
    [
      'a maintainer code in the consumer table',
      skill + '\n| `superseded-pending` | warning | x | y |\n',
      'consumer',
      'unexpected superseded-pending'
    ],
    [
      'a repeated code',
      skill + '\n| `mapping-missing` | error | x | y |\n',
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
      'a wrong audience',
      maintenance.replace(
        /^(\| `superseded-by-sync`\s*\|\s*warning\s*\|)\s*maintainer/m,
        '$1 consumer'
      ),
      'all',
      'superseded-by-sync audience consumer'
    ]
  ] as const)('should report %s', (_label, markdown, scope, problem) => {
    expect(tableProblems(markdown, scope)).toContain(problem)
  })
})
