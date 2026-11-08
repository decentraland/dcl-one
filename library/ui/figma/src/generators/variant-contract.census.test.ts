// Compile fixtures for the generated variant contracts. For every generated component whose axes are sparse, every
// caller prop object (each axis omitted or set to each captured value, booleans omitted, false or true) is type
// checked against the generated props type: it must compile exactly when the selection it defaults to is a drawn view.
// Spreads and wider-typed objects are checked too.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import { describe, expect, it } from 'vitest'
import { viewKey } from '../../review/src/components/figma-view-state.js'

const generatedDir = path.resolve(import.meta.dirname, '../../review/src/generated/components')
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

function contractOf(source: string) {
  return source.slice(source.indexOf('export type '), source.indexOf('\nexport const '))
}

// Type checks each statement against the generated props type; returns the indexes that do not compile.
// The diagnostics that mean "this value is not assignable to the props type". Any other diagnostic (a missing global,
// a syntax error) is a broken fixture, not a contract rejection, so it fails the test instead of counting as one.
const ASSIGNABILITY = new Set([2322, 2353, 2559, 2739, 2741, 2769])

function rejected(source: string, statements: string[]): Set<number> {
  const header = [
    'type ReactNode = string; type ForcedState = "hover" | "pressed";',
    'type HTMLElement = unknown; type HTMLAttributes<T> = { id?: string; className?: string }; type Omit<T, K> = T;',
    contractOf(source)
  ].join('\n')
  const text = `${header}\n${statements.join('\n')}`
  const input = ts.createSourceFile('c.ts', text, ts.ScriptTarget.Latest, true)
  const host = ts.createCompilerHost({ noLib: true })
  host.getSourceFile = (file) => (file === 'c.ts' ? input : undefined)
  const program = ts.createProgram(['c.ts'], { noLib: true, strict: true }, host)
  const offset = header.split('\n').length
  const lines = new Set<number>()
  for (const diagnostic of ts.getPreEmitDiagnostics(program)) {
    if (diagnostic.file?.fileName !== 'c.ts' || diagnostic.start === undefined) continue
    const line = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start).line - offset
    if (line < 0 || !ASSIGNABILITY.has(diagnostic.code))
      throw new Error(
        `Fixture diagnostic TS${diagnostic.code} on line ${line}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')}`
      )
    lines.add(line)
  }
  return lines
}

const sparse = [...sources]
  .filter(([, source]) => /\} & \(\n {2}\| \{/.test(source))
  .map(([name]) => name)
  .sort()

describe('generated variant contracts', () => {
  it('should cover the sparse components', () => {
    expect(sparse).toEqual(['NavBar', 'NavMenuDropdown', 'NavProfileButton', 'PlaceCard'])
  })

  it.each(sparse)('%s: compiles exactly the prop objects that default to a drawn view', (name) => {
    const source = sources.get(name)!
    const fn = source.slice(source.indexOf(`export const ${name} = ({`))
    const defaults: Record<string, string | boolean> = {}
    for (const m of fn.matchAll(/^ {2}(\w+) = (?:'((?:[^'\\]|\\.)*)'|(true|false))/gm))
      defaults[m[1]!] = m[2] ?? m[3] === 'true'
    const viewsBlock = source.slice(
      source.indexOf('Views: Record<string, ViewNode> = {'),
      source.indexOf('\nexport const ', source.indexOf('Views: Record<string, ViewNode> = {'))
    )
    const keys = new Set(
      [...viewsBlock.matchAll(/^ {2}'((?:[^'\\]|\\.)*)': \{/gm)].map((m) => m[1]!)
    )
    const values = /values: \{ ([^}]*) \}/.exec(fn)![1]!.split(', ').filter(Boolean)
    const booleans = (/booleans: \{ ([^}]*) \}/.exec(fn)?.[1] ?? '').split(', ').filter(Boolean)
    const domain = (axis: string) =>
      booleans.includes(axis)
        ? [false, true]
        : [
            ...new Set(
              [...keys].flatMap((key) =>
                key
                  .split('|')
                  .filter((part) => part.startsWith(`${axis}:`))
                  .map((part) => part.slice(axis.length + 1))
              )
            )
          ]

    // Each axis omitted, passed as explicit undefined, or set to each captured value (booleans false or true).
    const UNDEFINED = Symbol('undefined')
    let objects: Record<string, unknown>[] = [{}]
    for (const axis of [...values, ...booleans])
      objects = objects.flatMap((object) => [
        object,
        { ...object, [axis]: UNDEFINED },
        ...domain(axis).map((value) => ({ ...object, [axis]: value }))
      ])
    const literal = (object: Record<string, unknown>) =>
      `{ ${Object.entries(object)
        .map(
          ([key, value]) => `${key}: ${value === UNDEFINED ? 'undefined' : JSON.stringify(value)}`
        )
        .join(', ')} }`
    const valueOf = (object: Record<string, unknown>, axis: string) =>
      object[axis] === UNDEFINED || object[axis] === undefined ? defaults[axis] : object[axis]

    const renderable = objects.map((object) => {
      const selection = {
        values: Object.fromEntries(values.map((axis) => [axis, valueOf(object, axis) as string])),
        booleans: Object.fromEntries(
          booleans.map((axis) => [axis, valueOf(object, axis) as boolean])
        ),
        flags: {}
      }
      return keys.has(viewKey(selection))
    })
    const failures = rejected(
      source,
      objects.map((object, index) => `const t${index}: ${name}Props = ${literal(object)};`)
    )
    const mismatches = objects.filter((_, index) => renderable[index] === failures.has(index))
    expect(mismatches, `${name}: objects whose type check disagrees with the drawn views`).toEqual(
      []
    )
    expect(renderable.some(Boolean) && renderable.some((value) => !value)).toBe(true)
  })

  it('should reject spreads and wider-typed objects that reach an undrawn NavBar selection', () => {
    const source = sources.get('NavBar')!
    const statements = [
      // Positive: a drawn row, spread from a typed object, plus a standard element prop.
      'const drawn = { viewport: "mobile", signedIn: true, balances: "credits" } as const; const ok1: NavBarProps = { ...drawn, id: "nav" };',
      // Positive: a typed union member passed through.
      'declare const member: NavBarProps; const ok2: NavBarProps = { ...member };',
      // Negative: a spread that defaults into an undrawn tuple (signed-out with balances).
      'const undrawn = { balances: "credits" } as const; const bad1: NavBarProps = { ...undrawn };',
      // Negative: a wider string where the axis needs a literal.
      'declare const wide: string; const bad2: NavBarProps = { viewport: wide };',
      // Negative: a widened boolean that may select signedIn: true without balances support.
      'declare const signed: boolean; const bad4: NavBarProps = { signedIn: signed, balances: "credits" };'
    ]
    const failures = rejected(source, statements)
    expect([...failures].sort()).toEqual([2, 3, 4])
  })

  // An arbitrary Record<string, unknown> is not claimed to be rejected statically: its values are unknown, and the
  // runtime validates them (unknown values throw, reserved names are dropped). A fixture that refers to a missing
  // global must fail outright rather than pass as a rejection.
  it('should fail on a missing-global diagnostic instead of counting it as a contract rejection', () => {
    expect(() =>
      rejected(sources.get('NavBar')!, ['const missing: NavBarProps = notDeclaredAnywhere;'])
    ).toThrow('Fixture diagnostic TS2304')
  })
})
