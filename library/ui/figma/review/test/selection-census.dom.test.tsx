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
import { pickView as oldPickView } from './fixtures/figma-view-state-fd9b4576'
import oldDefaults from './fixtures/default-keys-fd9b4576.json'
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
  kind: 'spelling' | 'flags' | 'throw'
  props: Record<string, unknown>
  expectedKey: string
  oldOutcome: string
  newOutcome: string
}
const rows: Row[] = []
const picked = vi.mocked(viewState.pickView)

// Renders on the client and on the server and returns the view pickView chose in each.
// The old runtime's pickView, called with the exact views and selection the generated component passed to the new
// one, and the defaultKey the generated component carried before the change.
function oldOutcomeOf(name: string) {
  const call = picked.mock.calls.at(-1)!
  const defaultKey = (oldDefaults.defaultKeys as Record<string, string>)[name]!
  return oldPickView(call[0], defaultKey, call[1] as never)
}

// Renders on the client and on the server and returns the view pickView chose in each, plus the old runtime's choice
// for the same selection.
function chosen(
  name: string,
  Component: ComponentType<Record<string, unknown>>,
  props: Record<string, unknown>
) {
  picked.mockClear()
  render(<Component {...props} />)
  const client = picked.mock.results.at(-1)?.value
  const old = oldOutcomeOf(name)
  picked.mockClear()
  renderToString(<Component {...props} />)
  const server = picked.mock.results.at(-1)?.value
  cleanup()
  return { client, server, old }
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
        // Every subset of the default-valued axes left out, either omitted or passed as explicit undefined; the rest
        // given explicitly (false booleans included).
        for (let mask = 0; mask < 1 << defaulted.length; mask++)
          for (const absent of mask === 0 ? ['omit'] : ['omit', 'undefined']) {
            const props: Record<string, unknown> = { ...item.props }
            defaulted.forEach((axis, index) => {
              if (!(mask & (1 << index))) return
              if (absent === 'omit') delete props[axis]
              else props[axis] = undefined
            })
            const { client, server, old } = chosen(name, Component, props)
            expect(client, `${name} ${JSON.stringify(props)}`).toBe(views[item.key])
            expect(server, `${name} ${JSON.stringify(props)} (SSR)`).toBe(views[item.key])
            expect(old, `${name} ${JSON.stringify(props)} (old runtime)`).toBe(client)
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
      // dropping flags from the end, computed with the production viewKey. The public Button keeps its loading
      // view while unavailable; native disabled semantics are enforced separately from captured view selection.
      for (const item of cases.filter((entry) =>
        flagProps.every((flag) => entry.props[flag] !== true)
      )) {
        const base = Object.fromEntries(
          Object.entries(item.props).filter(([key]) => !FLAGS.includes(key as never))
        )
        // Mask 0, every flag off: omitted, explicit false and explicit undefined all resolve to the case view.
        if (flagProps.length)
          for (const off of [
            {},
            Object.fromEntries(flagProps.map((flag) => [flag, false])),
            Object.fromEntries(flagProps.map((flag) => [flag, undefined]))
          ]) {
            const props = { ...base, ...off }
            const { client, server, old } = chosen(name, Component, props)
            expect(client, `${name} ${JSON.stringify(props)}`).toBe(views[item.key])
            expect(server, `${name} ${JSON.stringify(props)} (SSR)`).toBe(views[item.key])
            expect(old, `${name} ${JSON.stringify(props)} (old runtime)`).toBe(client)
            rows.push({
              component: name,
              kind: 'flags',
              props,
              expectedKey: item.key,
              oldOutcome: 'exact',
              newOutcome: 'exact'
            })
          }
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
            flags:
              name === 'Button' && flags.loading === true ? { ...flags, disabled: false } : flags
          }
          const count = Object.values(selection.flags).filter(Boolean).length
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
          const { client, server, old } = chosen(name, Component, props)
          expect(client, `${name} ${JSON.stringify(props)}`).toBe(views[expectedKey])
          expect(server, `${name} ${JSON.stringify(props)} (SSR)`).toBe(views[expectedKey])
          expect(old, `${name} ${JSON.stringify(props)} (old runtime)`).toBe(client)
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

  // The selections that must fail: an unknown enum value, a non-boolean boolean axis, and an undrawn base tuple even
  // with interaction flags added (dropping flags never reaches another base). Each throws on the client and on the
  // server, where the old runtime returned the default view.
  it.each(components)(
    '%s: unknown values and undrawn bases throw instead of rendering another view',
    (name) => {
      const exports = generated as Record<string, unknown>
      const lower = name[0]!.toLowerCase() + name.slice(1)
      const Component = exports[name] as ComponentType<Record<string, unknown>>
      const views = exports[`${lower}Views`] as Record<string, unknown>
      const cases = exports[`${lower}Cases`] as Case[]
      const source = sources.get(name)!
      const fn = source.slice(source.indexOf(`export const ${name} = ({`))
      const flagProps = FLAGS.filter((flag) => new RegExp(`flags: \\{[^}]*\\b${flag}\\b`).test(fn))
      const first = Object.fromEntries(
        Object.entries(cases[0]!.props).filter(([key]) => !FLAGS.includes(key as never))
      )
      const attempts: Record<string, unknown>[] = []
      const enumAxis = Object.keys(first).find((axis) => typeof first[axis] === 'string')
      if (enumAxis) attempts.push({ ...first, [enumAxis]: '__not_captured__' })
      const booleanAxis = Object.keys(first).find((axis) => typeof first[axis] === 'boolean')
      if (booleanAxis) attempts.push({ ...first, [booleanAxis]: 'yes' })
      const axes = Object.keys(first)
      const domains = axes.map((axis) => [...new Set(cases.map((item) => item.props[axis]))])
      let combos: unknown[][] = [[]]
      for (const domain of domains)
        combos = combos.flatMap((combo) => domain.map((value) => [...combo, value]))
      const undrawn = combos
        .map((combo) => Object.fromEntries(axes.map((axis, index) => [axis, combo[index]])))
        .find((props) => {
          const selection = {
            values: Object.fromEntries(
              Object.entries(props).filter(([, value]) => typeof value === 'string')
            ) as Record<string, string>,
            booleans: Object.fromEntries(
              Object.entries(props).filter(([, value]) => typeof value === 'boolean')
            ) as Record<string, boolean>,
            flags: {}
          }
          return !(viewState.viewKey(selection) in views)
        })
      if (undrawn)
        attempts.push({ ...undrawn, ...Object.fromEntries(flagProps.map((flag) => [flag, true])) })
      for (const props of attempts) {
        const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
        try {
          expect(
            () => render(<Component {...props} />),
            `${name} ${JSON.stringify(props)}`
          ).toThrow(`${name} has no captured view for`)
          expect(
            () => renderToString(<Component {...props} />),
            `${name} ${JSON.stringify(props)} (SSR)`
          ).toThrow(`${name} has no captured view for`)
        } finally {
          log.mockRestore()
          cleanup()
        }
        // The old runtime, given the same selection, rendered a view instead: the default one, or for a coerced
        // non-boolean the view of the truthy value.
        const old = oldOutcomeOf(name)
        const defaultKey = (oldDefaults.defaultKeys as Record<string, string>)[name]!
        const oldKey = Object.keys(views).find((key) => views[key] === old)
        expect(oldKey, `${name} ${JSON.stringify(props)} (old runtime)`).toBeDefined()
        rows.push({
          component: name,
          kind: 'throw',
          props,
          expectedKey: '',
          oldOutcome: oldKey === defaultKey ? `default-fallback:${oldKey}` : `coerced:${oldKey}`,
          newOutcome: 'throw'
        })
      }
      // A component with no variant axes has no value a caller could get wrong.
      if (!attempts.length) expect(axes, name).toEqual([])
    }
  )

  it('should throw for empty views, with or without flags, in the old and the new runtime', () => {
    for (const flags of [{}, { disabled: true }]) {
      expect(() => viewState.pickView({}, { values: {}, booleans: {}, flags }, 'Empty')).toThrow(
        'Empty has no captured view for'
      )
      expect(() => oldPickView({}, 'default', { values: {}, booleans: {}, flags })).toThrow(
        'A generated component must have at least one view'
      )
    }
  })

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
        throws: rows.filter((row) => row.kind === 'throw').length,
        nestedReferences: 0
      },
      rows
    }
    writeFileSync(out, JSON.stringify(report, null, 1))
  })
})
