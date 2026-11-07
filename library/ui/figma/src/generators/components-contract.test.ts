import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { ESLint } from 'eslint'
import ts from 'typescript'
import { beforeEach, describe, expect, it } from 'vitest'
import { projectRoot } from '../../test/snapshot.js'
import { loadConfig } from '../config.js'
import type { ComponentSpec } from '../ir/components.js'
import { buildComponentSpecs } from '../mappers/component-spec.js'
import { generateComponents } from './components.js'

async function navbarSpec(): Promise<ComponentSpec> {
  const config = await loadConfig(projectRoot)
  const data = JSON.parse(
    await readFile(path.join(projectRoot, 'figma-snapshot/components/Nav bar.json'), 'utf8')
  )
  return buildComponentSpecs({
    pages: [{ name: 'Nav bar', category: 'Navigation', data }],
    config,
    collections: [],
    textStyles: [],
    icons: new Map()
  }).find((spec) => spec.name === 'NavBar')!
}

function sourceFor(spec: ComponentSpec): string {
  return generateComponents([spec], {}).files.find((file) =>
    file.path.endsWith(`/${spec.name}.tsx`)
  )!.content
}

// Lints a generated component as the library lints promoted ones, at components/<Name>/<Name>.tsx.
async function lintAsPromoted(name: string, source: string) {
  const library = path.resolve(projectRoot, '..')
  const [result] = await new ESLint({ cwd: library }).lintText(source, {
    filePath: path.join(library, 'components', name, `${name}.tsx`)
  })
  return result!.messages.map((message) => `${message.ruleId}: ${message.message}`)
}

function contractDiagnostics(source: string, assignments: string): string[] {
  const contract = source.slice(source.indexOf('export type '), source.indexOf('\nexport const '))
  const text = [
    'type ReactNode = string; type ForcedState = "hover" | "pressed";',
    'type HTMLElement = unknown; type HTMLAttributes<T> = {}; type Omit<T, K> = T;',
    contract,
    assignments
  ].join('\n')
  const file = 'contract.ts'
  const input = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true)
  const host = ts.createCompilerHost({ noLib: true })
  host.getSourceFile = (name) => (name === file ? input : undefined)
  const program = ts.createProgram([file], { noLib: true, strict: true }, host)
  return ts
    .getPreEmitDiagnostics(program)
    .filter((diagnostic) => diagnostic.file?.fileName === file)
    .map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))
}

describe('generated public component contracts', () => {
  describe('when NavBar has sparse captured variant combinations', () => {
    let spec: ComponentSpec
    let source: string
    let valid: string
    let invalid: string

    beforeEach(async () => {
      spec = await navbarSpec()
      source = sourceFor(spec)
      valid = ''
      invalid = ''
      for (const viewport of ['largeDesktop', 'desktop', 'mobile']) {
        for (const signedIn of [false, true]) {
          for (const balances of ['none', 'credits', 'creditsMana']) {
            for (const theme of ['dark', 'light']) {
              const props = { viewport, signedIn, balances, theme }
              const line = `const v${viewport}${signedIn}${balances}${theme}: NavBarProps = ${JSON.stringify(props)};\n`
              if (balances === 'none' || (signedIn && theme === 'dark')) valid += line
              else invalid += '// @ts-expect-error uncaptured variant combination\n' + line
            }
          }
        }
      }
    })

    it('should accept all eighteen captured combinations', () => {
      expect(contractDiagnostics(source, valid)).toEqual([])
    })

    it('should reject all eighteen combinations absent from Figma', () => {
      expect(contractDiagnostics(source, invalid)).toEqual([])
    })

    it('should accept omitted defaults and reject balances without signing in', () => {
      expect(
        contractDiagnostics(
          source,
          [
            'const defaults: NavBarProps = {};',
            'const credits: NavBarProps = { signedIn: true, balances: "credits" };',
            '// @ts-expect-error default signedIn is false',
            'const invalid: NavBarProps = { balances: "credits" };'
          ].join('\n')
        )
      ).toEqual([])
    })
  })

  describe('when public text overrides are disabled for a component', () => {
    let source: string
    let spec: ComponentSpec

    beforeEach(async () => {
      spec = await navbarSpec()
      source = sourceFor(spec)
    })

    it('should carry the opt-out from configuration through the mapper', () => {
      expect(spec.exposeTextSlots).toBe(false)
    })

    it('should reject positional text override props', () => {
      expect(
        contractDiagnostics(
          source,
          [
            '// @ts-expect-error positional text labels have different roles in each view',
            'const unsafe: NavBarProps = { label: "Explore" };',
            '// @ts-expect-error later positional labels are not public either',
            'const unsafeLater: NavBarProps = { label8: "Explore" };'
          ].join('\n')
        )
      ).toEqual([])
    })

    it('should pass no text override values to the view runtime', () => {
      expect(source).toContain('slots={{}}')
    })

    it('should hand every suppressed text prop to the runtime to drop, binding none of them', () => {
      const omitted = spec.slots.filter((slot) => slot.kind === 'text').map((slot) => slot.name)
      expect(omitted.length).toBeGreaterThan(1)
      expect(source).toContain(
        `suppressedProps={[${omitted.map((name) => `'${name}'`).join(', ')}]}`
      )
      expect(source).not.toMatch(/\b_label\d*\b/)
    })

    it('should pass the library lint once promoted', async () => {
      expect(await lintAsPromoted('NavBar', source)).toEqual([])
    })

    it('should fail that lint if a suppressed prop were bound to an unused alias (control)', async () => {
      const aliased = source.replace('  forceState,\n', '  label: _label,\n  forceState,\n')
      expect(aliased).not.toBe(source)
      expect(await lintAsPromoted('NavBar', aliased)).toContainEqual(
        expect.stringContaining('@typescript-eslint/no-unused-vars')
      )
    })

    it('should regenerate byte-identically from a fresh capture read', async () => {
      expect(sourceFor(await navbarSpec())).toBe(source)
    })

    it('should name the component for the runtime error on an uncaptured selection', () => {
      expect(source).toContain("component='NavBar'")
    })

    it('should retain semantic link targets independently of text overrides', () => {
      expect(source).toContain('labelHref?: string')
    })

    it('should preserve icon overrides', () => {
      expect(
        sourceFor({ ...spec, slots: [...spec.slots, { name: 'icon', kind: 'icon' }] })
      ).toContain('slots={{ icon }}')
    })
  })

  describe('when a component keeps the default public text API', () => {
    let spec: ComponentSpec

    beforeEach(async () => {
      spec = await navbarSpec()
      delete spec.exposeTextSlots
    })

    it('should suppress nothing when every text override is public', () => {
      expect(sourceFor(spec)).not.toContain('suppressedProps')
    })

    it('should preserve text overrides if the setting is omitted', () => {
      expect(
        contractDiagnostics(sourceFor(spec), 'const props: NavBarProps = { label: "Explore" };')
      ).toEqual([])
    })

    it('should preserve text overrides if the setting is explicitly enabled', () => {
      expect(
        contractDiagnostics(
          sourceFor({ ...spec, exposeTextSlots: true }),
          'const props: NavBarProps = { label: "Explore" };'
        )
      ).toEqual([])
    })
  })
})

describe('when a variant value contains a line terminator', () => {
  it('should emit a parsable union type', async () => {
    const spec = await navbarSpec()
    const prop = spec.props.find((item) => item.kind === 'enum')!
    const hostile = 'one\ntwo\rthree\u2028four\u2029five'
    prop.default = hostile
    for (const view of spec.views) view.values[prop.name] = hostile
    const source = sourceFor(spec)
    const parsed = ts.createSourceFile(
      'generated.tsx',
      source,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX
    )
    expect((parsed as unknown as { parseDiagnostics: unknown[] }).parseDiagnostics).toEqual([])
  })
})

describe('when a JavaScript caller supplies suppressed text slots', () => {
  // The runtime drops them before the DOM (figma-runtime.dom.test.tsx renders that); the type forbids them.
  it('should type each as never and list it for the runtime to drop', async () => {
    const spec = await navbarSpec()
    const source = sourceFor(spec)
    const suppressed = /suppressedProps=\{\[([^\]]*)\]\}/.exec(source)?.[1] ?? ''
    for (const slot of spec.slots.filter((item) => item.kind === 'text')) {
      expect(suppressed).toContain(`'${slot.name}'`)
      expect(source).toContain(`${slot.name}?: never`)
    }
  })
})
