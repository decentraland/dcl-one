// Selection census through the generated components and the production runtime. For every drawn catalog case it
// renders every spelling a caller can use for that selection (each default-valued axis omitted or given explicitly,
// booleans included), and every interaction-flag subset over each flag-free case, on the client and on the server.
// pickView is wrapped, not replaced, so each render's chosen view is compared by identity with the generated view for
// the expected key. Set CENSUS_OUT to also write a provenance-bound JSON report.
import { cleanup, render } from '@testing-library/react'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import type { ComponentType } from 'react'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import * as viewState from '../src/components/figma-view-state'
import * as generated from '../src/generated/components'

vi.mock('../src/components/figma-view-state', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../src/components/figma-view-state')>()
  return { ...actual, pickView: vi.fn(actual.pickView) }
})

afterEach(() => cleanup())

const review = path.resolve(import.meta.dirname, '..')
const figma = path.resolve(review, '..')
const generatedDir = path.join(review, 'src/generated/components')
const FLAGS = ['selected', 'disabled', 'loading'] as const

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name)
    return statSync(full).isDirectory() ? files(full) : [full]
  })
}
const sources = new Map(
  files(generatedDir)
    .filter((file) => file.endsWith('.tsx'))
    .map((file) => [path.basename(file, '.tsx'), readFileSync(file, 'utf8')])
)

type Case = { key: string; props: Record<string, unknown> }
type Row = {
  component: string
  kind: 'spelling' | 'flags'
  props: Record<string, unknown>
  expectedKey: string
  oldOutcome: string
  newOutcome: string
}
const rows: Row[] = []
const picked = vi.mocked(viewState.pickView)

// Renders on the client and on the server and returns the view pickView chose in each.
function chosen(Component: ComponentType<Record<string, unknown>>, props: Record<string, unknown>) {
  picked.mockClear()
  render(<Component {...props} />)
  const client = picked.mock.results.at(-1)?.value
  picked.mockClear()
  renderToString(<Component {...props} />)
  const server = picked.mock.results.at(-1)?.value
  cleanup()
  return { client, server }
}

// Every generated component file; the module namespace is read inside each test, once the mocked runtime has loaded.
const components = [...sources.keys()].sort()

describe('selection census over every generated component', () => {
  it('should have no nested generated references', () => {
    for (const [name, source] of sources) expect(source.includes("kind: 'ref'"), name).toBe(false)
  })

  it.each(components)(
    '%s: every spelling of every drawn case and every flag subset picks the expected view',
    (name) => {
      const exports = generated as Record<string, unknown>
      const lower = name[0]!.toLowerCase() + name.slice(1)
      const Component = exports[name] as ComponentType<Record<string, unknown>>
      const views = exports[`${lower}Views`] as Record<string, unknown>
      const cases = exports[`${lower}Cases`] as Case[]
      const source = sources.get(name)!
      const fn = source.slice(source.indexOf(`export const ${name} = ({`))
      const defaults: Record<string, unknown> = {}
      for (const m of fn.matchAll(/^ {2}(\w+) = (?:'((?:[^'\\]|\\.)*)'|(true|false))/gm))
        defaults[m[1]!] = m[2] ?? m[3] === 'true'
      const flagProps = FLAGS.filter((flag) => new RegExp(`flags: \\{[^}]*\\b${flag}\\b`).test(fn))

      for (const item of cases) {
        const axes = Object.keys(item.props).filter((key) => !FLAGS.includes(key as never))
        const defaulted = axes.filter((axis) => item.props[axis] === defaults[axis])
        // Every subset of the default-valued axes omitted; the rest given explicitly (false booleans included).
        for (let mask = 0; mask < 1 << defaulted.length; mask++) {
          const props = { ...item.props }
          defaulted.forEach((axis, index) => {
            if (mask & (1 << index)) delete props[axis]
          })
          const { client, server } = chosen(Component, props)
          expect(client, `${name} ${JSON.stringify(props)}`).toBe(views[item.key])
          expect(server, `${name} ${JSON.stringify(props)} (SSR)`).toBe(views[item.key])
          rows.push({
            component: name,
            kind: 'spelling',
            props,
            expectedKey: item.key,
            oldOutcome: item.key,
            newOutcome: item.key
          })
        }
      }

      // Every non-empty flag subset over each flag-free case: the expected view is the first generated key found by
      // dropping flags from the end, computed with the production viewKey.
      for (const item of cases.filter((entry) =>
        flagProps.every((flag) => entry.props[flag] !== true)
      )) {
        const base = Object.fromEntries(
          Object.entries(item.props).filter(([key]) => !FLAGS.includes(key as never))
        )
        for (let mask = 1; mask < 1 << flagProps.length; mask++) {
          const flags = Object.fromEntries(
            flagProps.filter((_, index) => mask & (1 << index)).map((flag) => [flag, true])
          )
          const selection = {
            values: Object.fromEntries(
              Object.entries(base).filter(([, value]) => typeof value === 'string')
            ) as Record<string, string>,
            booleans: Object.fromEntries(
              Object.entries(base).filter(([, value]) => typeof value === 'boolean')
            ) as Record<string, boolean>,
            flags
          }
          const count = Object.keys(flags).length
          let expectedKey = ''
          let dropped = 0
          for (; dropped <= count; dropped++) {
            const key = viewState.viewKey(selection, dropped)
            if (key in views) {
              expectedKey = key
              break
            }
          }
          expect(expectedKey, `${name} ${JSON.stringify(flags)} resolves`).not.toBe('')
          const props = { ...base, ...flags }
          const { client, server } = chosen(Component, props)
          expect(client, `${name} ${JSON.stringify(props)}`).toBe(views[expectedKey])
          expect(server, `${name} ${JSON.stringify(props)} (SSR)`).toBe(views[expectedKey])
          const outcome = dropped === 0 ? 'exact' : `flag-drop:${dropped}`
          rows.push({
            component: name,
            kind: 'flags',
            props,
            expectedKey,
            oldOutcome: outcome,
            newOutcome: outcome
          })
        }
      }
    }
  )

  it('should write the provenance-bound report when CENSUS_OUT is set', () => {
    const out = process.env.CENSUS_OUT
    if (!out) return
    const sha = (data: string | Buffer) => createHash('sha256').update(data).digest('hex')
    const tree = (dir: string) =>
      sha(
        files(dir)
          .sort()
          .map((file) => `${path.relative(dir, file)} ${sha(readFileSync(file))}`)
          .join('\n')
      )
    const git = (...args: string[]) =>
      execFileSync('git', ['-C', figma, ...args], { encoding: 'utf8' }).trim()
    const report = {
      provenance: {
        commit: git('rev-parse', 'HEAD'),
        tree: git('rev-parse', 'HEAD^{tree}'),
        dirtyPaths: git('status', '--porcelain').split('\n').filter(Boolean).length,
        configSha256: sha(readFileSync(path.join(figma, 'figma.config.json'))),
        snapshotTreeSha256: tree(path.join(figma, 'figma-snapshot')),
        generatedTreeSha256: tree(generatedDir),
        runtimeSha256: tree(path.join(review, 'src/components')),
        testSha256: sha(readFileSync(new URL(import.meta.url))),
        command: 'CENSUS_OUT=<file> pnpm vitest run figma/review/test/selection-census.dom.test.tsx'
      },
      totals: {
        components: components.length,
        spellings: rows.filter((row) => row.kind === 'spelling').length,
        flagSelections: rows.filter((row) => row.kind === 'flags').length,
        flagExact: rows.filter((row) => row.kind === 'flags' && row.newOutcome === 'exact').length,
        flagDropped: rows.filter((row) => row.kind === 'flags' && row.newOutcome !== 'exact')
          .length,
        nestedReferences: 0
      },
      rows
    }
    writeFileSync(out, JSON.stringify(report, null, 1))
  })
})
