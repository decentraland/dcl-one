// Compile fixtures at the real public boundary: one TypeScript program with the Review's own tsconfig, the React
// types and the exported `<Name>Props` of the generated components (no sliced contract, no stubbed HTMLAttributes).
// Every prop object of each sparse component must compile exactly when it defaults to a drawn view, and JSX with the
// documented public props, spreads and wider types must compile or fail as stated.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import { beforeAll, describe, expect, it } from 'vitest'
import { viewKey } from '../../review/src/components/figma-view-state.js'

const review = path.resolve(import.meta.dirname, '../../review')
const generatedDir = path.join(review, 'src/generated/components')
const virtualFile = path.join(review, 'test/__public_props__.tsx')

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name)
    return statSync(full).isDirectory() ? files(full) : [full]
  })
}
const generatedFiles = new Map(
  files(generatedDir)
    .filter((file) => file.endsWith('.tsx'))
    .map((file) => [path.basename(file, '.tsx'), file])
)
const SPARSE = ['NavBar', 'NavMenuDropdown', 'NavProfileButton', 'PlaceCard']
// Only these mean "not assignable to the props"; any other diagnostic is a broken fixture and fails the run.
const ASSIGNABILITY = new Set([2322, 2353, 2559, 2739, 2741, 2769])
const UNDEFINED = Symbol('undefined')

type Fixture = { component: string; line: number; text: string; compiles: boolean; why: string }

// The prop objects of one component: each axis omitted, explicit undefined, or each captured value (booleans false or
// true), and whether each defaults to a drawn view according to the generated keys.
function enumerate(name: string): { text: string; compiles: boolean }[] {
  const source = readFileSync(generatedFiles.get(name)!, 'utf8')
  const fn = source.slice(source.indexOf(`export const ${name} = ({`))
  const defaults: Record<string, string | boolean> = {}
  for (const m of fn.matchAll(/^ {2}(\w+) = (?:'((?:[^'\\]|\\.)*)'|(true|false))/gm))
    defaults[m[1]!] = m[2] ?? m[3] === 'true'
  const block = source.slice(
    source.indexOf('Views: Record<string, ViewNode> = {'),
    source.indexOf('\nexport const ', source.indexOf('Views: Record<string, ViewNode> = {'))
  )
  const keys = new Set([...block.matchAll(/^ {2}'((?:[^'\\]|\\.)*)': \{/gm)].map((m) => m[1]!))
  const values = /values: \{ ([^}]*) \}/.exec(fn)![1]!.split(', ').filter(Boolean)
  const booleans = (/booleans: \{ ([^}]*) \}/.exec(fn)?.[1] ?? '').split(', ').filter(Boolean)
  const domain = (axis: string): unknown[] =>
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
  let objects: Record<string, unknown>[] = [{}]
  for (const axis of [...values, ...booleans])
    objects = objects.flatMap((object) => [
      object,
      { ...object, [axis]: UNDEFINED },
      ...domain(axis).map((value) => ({ ...object, [axis]: value }))
    ])
  const valueOf = (object: Record<string, unknown>, axis: string) =>
    object[axis] === UNDEFINED || object[axis] === undefined ? defaults[axis] : object[axis]
  // A responsive axis left unset renders every option's view, so the object is drawn only when each of them is.
  const responsive = /responsive=\{\{ prop: '(\w+)'/.exec(fn)?.[1]
  const selections = (object: Record<string, unknown>) =>
    responsive && valueOf(object, responsive) === undefined
      ? domain(responsive).map((option) => ({ ...object, [responsive]: option }))
      : [object]
  return objects.map((object) => ({
    text: `{ ${Object.entries(object)
      .map(([key, value]) => `${key}: ${value === UNDEFINED ? 'undefined' : JSON.stringify(value)}`)
      .join(', ')} }`,
    compiles: selections(object).every((selected) =>
      keys.has(
        viewKey({
          values: Object.fromEntries(
            values.map((axis) => [axis, valueOf(selected, axis) as string])
          ),
          booleans: Object.fromEntries(
            booleans.map((axis) => [axis, valueOf(selected, axis) as boolean])
          ),
          flags: {}
        })
      )
    )
  }))
}

// JSX at the public boundary for NavBar. A Record<string, unknown> spread is deliberately not asserted: its values are
// not statically known, which is what the runtime's throws and attribute isolation cover.
const NAV_BAR_JSX: { text: string; compiles: boolean; why: string }[] = [
  {
    text: '<NavBar viewport="mobile" signedIn balances="credits" className="x" style={{ color: "red" }} onClick={() => undefined} aria-label="Main" data-testid="bar" forceState="hover" />',
    compiles: true,
    why: 'a drawn row with the documented public props'
  },
  {
    text: '<NavBar {...({ viewport: "mobile", signedIn: true, balances: "credits" } as const)} id="nav" />',
    compiles: true,
    why: 'a typed spread of a drawn row plus an HTML attribute'
  },
  { text: '<NavBar {...member} />', compiles: true, why: 'a spread of a typed union member' },
  {
    text: '<NavBar balances="credits" />',
    compiles: false,
    why: 'defaults into signed-out with balances, undrawn'
  },
  {
    text: '<NavBar {...({ balances: "credits" } as const)} />',
    compiles: false,
    why: 'a spread that defaults into an undrawn tuple'
  },
  {
    text: '<NavBar viewport={wide} />',
    compiles: false,
    why: 'a wider string where the axis needs a literal'
  },
  {
    text: '<NavBar signedIn={flag} balances="credits" />',
    compiles: false,
    why: 'a widened boolean that can select an undrawn tuple'
  },
  {
    text: '<NavBar label="Explore" />',
    compiles: false,
    why: 'a private text override (typed never)'
  }
]

let fixtures: Fixture[] = []
let failing = new Set<number>()

beforeAll(() => {
  const config = ts.readConfigFile(path.join(review, 'tsconfig.json'), ts.sys.readFile)
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, review)
  const lines: string[] = []
  const imports = SPARSE.map((name) => {
    const relative = path
      .relative(path.dirname(virtualFile), generatedFiles.get(name)!)
      .replace(/\.tsx$/, '')
    return `import { ${name}, type ${name}Props } from './${relative}'`
  })
  lines.push(
    ...imports,
    'declare const member: NavBarProps',
    'declare const wide: string',
    'declare const flag: boolean'
  )
  const header = lines.length
  const list: Fixture[] = []
  for (const name of SPARSE)
    for (const object of enumerate(name)) {
      list.push({
        component: name,
        line: lines.length,
        text: object.text,
        compiles: object.compiles,
        why: 'enumerated'
      })
      lines.push(`export const t${lines.length}: ${name}Props = ${object.text}`)
    }
  for (const item of NAV_BAR_JSX) {
    list.push({ component: 'NavBar', line: lines.length, ...item })
    lines.push(`export const j${lines.length} = ${item.text}`)
  }
  const text = lines.join('\n')
  const host = ts.createCompilerHost(parsed.options)
  const base = {
    getSourceFile: host.getSourceFile,
    fileExists: host.fileExists,
    readFile: host.readFile
  }
  host.getSourceFile = (file, version, ...rest) =>
    path.resolve(file) === virtualFile
      ? ts.createSourceFile(file, text, version, true, ts.ScriptKind.TSX)
      : base.getSourceFile(file, version, ...rest)
  host.fileExists = (file) => path.resolve(file) === virtualFile || base.fileExists(file)
  host.readFile = (file) => (path.resolve(file) === virtualFile ? text : base.readFile(file))
  const program = ts.createProgram(
    [...parsed.fileNames.filter((file) => !file.includes('/test/')), virtualFile],
    parsed.options,
    host
  )
  const bad = new Set<number>()
  for (const diagnostic of ts.getPreEmitDiagnostics(program, program.getSourceFile(virtualFile))) {
    if (!diagnostic.file || diagnostic.start === undefined) continue
    const line = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start).line
    if (line < header || !ASSIGNABILITY.has(diagnostic.code))
      throw new Error(
        `Fixture diagnostic TS${diagnostic.code} on line ${line}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')}`
      )
    bad.add(line)
  }
  fixtures = list
  failing = bad
}, 120_000)

describe('generated props at the real public boundary', () => {
  it.each(SPARSE)(
    '%s: every prop object compiles exactly when it defaults to a drawn view',
    (name) => {
      const own = fixtures.filter((item) => item.component === name && item.why === 'enumerated')
      const mismatches = own
        .filter((item) => item.compiles === failing.has(item.line))
        .map((item) => item.text)
      expect(mismatches).toEqual([])
      expect(own.some((item) => item.compiles) && own.some((item) => !item.compiles)).toBe(true)
    }
  )

  it.each(NAV_BAR_JSX.map((item) => [item.why, item.compiles] as const))(
    'NavBar JSX: %s should %s',
    (why, compiles) => {
      const item = fixtures.find((entry) => entry.why === why)!
      expect(!failing.has(item.line)).toBe(compiles)
    }
  )
})
