import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { ESLint } from 'eslint'
import ts from 'typescript'
import { beforeEach, describe, expect, it } from 'vitest'
import { fixturesDir, projectRoot, snapshotDir, testConfig } from '../../test/snapshot.js'
import type { ComponentSpec, TextRole, ViewNode } from '../ir/components.js'
import { buildComponentSpecs, withTextRoles } from '../mappers/component-spec.js'
import { generateComponents } from './components.js'

async function demoNavSpec(): Promise<ComponentSpec> {
  const config = await testConfig()
  const data = JSON.parse(
    await readFile(path.join(snapshotDir, 'components/Demo Nav.json'), 'utf8')
  )
  return buildComponentSpecs({
    pages: [{ name: 'Demo Nav', category: 'Navigation', data }],
    config,
    collections: [],
    textStyles: [],
    icons: new Map()
  }).find((spec) => spec.name === 'DemoNav')!
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
  describe('when DemoNav has sparse captured variant combinations', () => {
    let spec: ComponentSpec
    let source: string
    let valid: string
    let invalid: string

    beforeEach(async () => {
      spec = await demoNavSpec()
      source = sourceFor(spec)
      valid = ''
      invalid = ''
      for (const viewport of ['largeDesktop', 'desktop', 'mobile']) {
        for (const signedIn of [false, true]) {
          for (const balances of ['none', 'credits', 'creditsMana']) {
            for (const theme of ['dark', 'light']) {
              const props = { viewport, signedIn, balances, theme }
              const line = `const v${viewport}${signedIn}${balances}${theme}: DemoNavProps = ${JSON.stringify(props)};\n`
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
            'const defaults: DemoNavProps = {};',
            'const credits: DemoNavProps = { signedIn: true, balances: "credits" };',
            '// @ts-expect-error default signedIn is false',
            'const invalid: DemoNavProps = { balances: "credits" };'
          ].join('\n')
        )
      ).toEqual([])
    })
  })

  describe('when public text overrides are disabled for a component', () => {
    let source: string
    let spec: ComponentSpec

    beforeEach(async () => {
      spec = await demoNavSpec()
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
            'const unsafe: DemoNavProps = { label: "Explore" };',
            '// @ts-expect-error later positional labels are not public either',
            'const unsafeLater: DemoNavProps = { label8: "Explore" };'
          ].join('\n')
        )
      ).toEqual([])
    })

    it('should pass no text override values to the view runtime', () => {
      expect(source).toContain('slots={{}}')
    })

    it('should hand every suppressed text prop to the runtime as a module constant, binding none of them', () => {
      const omitted = spec.slots.filter((slot) => slot.kind === 'text').map((slot) => slot.name)
      expect(omitted.length).toBeGreaterThan(1)
      expect(source).toContain(
        `const demoNavSuppressedProps = Object.freeze([${omitted.map((name) => `'${name}'`).join(', ')}] as const)`
      )
      expect(source).toContain('suppressed={demoNavSuppressedProps}')
      expect(source).not.toMatch(/\b_label\d*\b/)
    })

    it('should pass the caller props as data instead of spreading them over the generated configuration', () => {
      expect(source).toContain('attributes={rest}')
      expect(source).not.toContain('{...rest}')
      expect(source).not.toContain('suppressedProps=')
    })

    it('should pass the library lint once promoted', async () => {
      expect(await lintAsPromoted('DemoNav', source)).toEqual([])
    })

    it('should fail that lint if a suppressed prop were bound to an unused alias (control)', async () => {
      const aliased = source.replace('  forceState,\n', '  label: _label,\n  forceState,\n')
      expect(aliased).not.toBe(source)
      expect(await lintAsPromoted('DemoNav', aliased)).toContainEqual(
        expect.stringContaining('@typescript-eslint/no-unused-vars')
      )
    })

    it('should regenerate byte-identically from a fresh capture read', async () => {
      expect(sourceFor(await demoNavSpec())).toBe(source)
    })

    it('should name the component for the runtime error on an uncaptured selection', () => {
      expect(source).toContain("component='DemoNav'")
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
      spec = await demoNavSpec()
      delete spec.exposeTextSlots
    })

    it('should suppress nothing when every text override is public', () => {
      const source = sourceFor(spec)
      expect(source).not.toContain('SuppressedProps')
      expect(source).not.toContain('suppressed=')
    })

    it('should preserve text overrides if the setting is omitted', () => {
      expect(
        contractDiagnostics(sourceFor(spec), 'const props: DemoNavProps = { label: "Explore" };')
      ).toEqual([])
    })

    it('should preserve text overrides if the setting is explicitly enabled', () => {
      expect(
        contractDiagnostics(
          sourceFor({ ...spec, exposeTextSlots: true }),
          'const props: DemoNavProps = { label: "Explore" };'
        )
      ).toEqual([])
    })
  })
})

describe('when a variant value contains a line terminator', () => {
  it('should emit a parsable union type', async () => {
    const spec = await demoNavSpec()
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
    const spec = await demoNavSpec()
    const source = sourceFor(spec)
    const suppressed =
      /const demoNavSuppressedProps = Object\.freeze\(\[([^\]]*)\] as const\)/.exec(source)?.[1] ??
      ''
    for (const slot of spec.slots.filter((item) => item.kind === 'text')) {
      expect(suppressed).toContain(`'${slot.name}'`)
      expect(source).toContain(`${slot.name}?: never`)
    }
  })
})

describe('when a component binds text roles', () => {
  let source: string

  // The fixture ids written onto the synthetic DemoNav views, as a capture that records descendant ids would.
  // The four menu tabs are links, so their roles declare the link prop names.
  const HREFS: Record<string, string> = {
    discoverLabel: 'discoverHref',
    shopLabel: 'shopHref',
    createLabel: 'createHref',
    learnLabel: 'learnHref'
  }

  async function demoNavWithRoles(hrefs: Record<string, string> = HREFS): Promise<ComponentSpec> {
    const spec = await demoNavSpec()
    const { targets } = JSON.parse(
      await readFile(path.join(fixturesDir, 'demo-nav-text-roles.json'), 'utf8')
    ) as {
      targets: { role: string; view: string; path: string; nodeId: string; namePath: string[] }[]
    }
    const roles: Record<string, TextRole> = {
      notificationsLabel: { type: 'string', nodes: [] }
    }
    for (const target of targets) {
      let node: ViewNode = spec.views.find((view) => view.key === target.view)!.root
      for (const index of target.path.split('.').map(Number)) node = node.children![index]!
      node.figmaId = target.nodeId
      roles[target.role] ??=
        target.role === 'notificationCount'
          ? { type: 'count', label: 'notificationsLabel', nodes: [] }
          : {
              type: 'string',
              nodes: [],
              ...(hrefs[target.role] ? { href: hrefs[target.role] } : {})
            }
      roles[target.role]!.nodes.push({ nodeId: target.nodeId, namePath: target.namePath })
    }
    return withTextRoles(spec, roles)
  }

  beforeEach(async () => {
    source = sourceFor(await demoNavWithRoles())
  })

  it('should type each string role as an optional string prop', () => {
    for (const name of [
      'signInLabel',
      'discoverLabel',
      'shopLabel',
      'createLabel',
      'learnLabel',
      'creditsBalance',
      'manaBalance',
      'polygonManaBalance'
    ])
      expect(source).toContain(`  ${name}?: string`)
  })

  it('should never emit a Figma node id', () => {
    expect(source).not.toContain('90001:')
    expect(source).not.toContain('figmaId')
  })

  it('should pass the role values and the count label to the runtime', () => {
    expect(source).toMatch(/slots=\{\{ [^}]*signInLabel[^}]*notificationCount[^}]*\}\}/)
    expect(source).toContain('counts={{ notificationCount: notificationsLabel }}')
  })

  it('should leave no positional text override once every text layer has a role', () => {
    expect(source).not.toMatch(/^ {2}label\d*\?:/m)
    expect(source).not.toContain('suppressed=')
  })

  it('should emit exactly the link prop names the roles declare', () => {
    for (const href of ['discoverHref', 'shopHref', 'createHref', 'learnHref'])
      expect(source).toContain(`  ${href}?: string`)
    expect(source).not.toMatch(/labelHref|LabelHref/)
  })

  it('should fail generation when a declared href takes the name of a semantic handler', async () => {
    const spec = await demoNavWithRoles({ ...HREFS, shopLabel: 'onMenu' })
    expect(() => sourceFor(spec)).toThrow('DemoNav: generated prop onMenu is declared twice')
  })

  it('should fail generation when a role under a linked action declares no href', async () => {
    const { shopLabel: _shop, ...withoutShop } = HREFS
    void _shop
    const spec = await demoNavWithRoles(withoutShop)
    expect(() => sourceFor(spec)).toThrow(
      'Text role shopLabel sits under the linked action MenuTab: declare its href in textRoles'
    )
  })

  it('should accept a count with its label, or neither', () => {
    expect(
      contractDiagnostics(
        source,
        [
          'const none: DemoNavProps = { signInLabel: "Entrar" };',
          'const both: DemoNavProps = { signedIn: true, notificationCount: 3, notificationsLabel: "3 notificaciones" };'
        ].join('\n')
      )
    ).toEqual([])
  })

  it('should reject a count without its label and a count that is not a number', () => {
    expect(
      contractDiagnostics(
        source,
        [
          '// @ts-expect-error a count needs its accessible label',
          'const bare: DemoNavProps = { signedIn: true, notificationCount: 3 };',
          '// @ts-expect-error a count is a number',
          'const text: DemoNavProps = { signedIn: true, notificationCount: "3", notificationsLabel: "3" };',
          '// @ts-expect-error a string role takes text, not a number',
          'const label: DemoNavProps = { signInLabel: 3 };'
        ].join('\n')
      )
    ).toEqual([])
  })

  it('should pass the library lint once promoted', async () => {
    expect(await lintAsPromoted('DemoNav', source)).toEqual([])
  })
})

describe('when figma.config.json semantics name a prop that is not a valid binding', () => {
  it.each([
    ['href', 'default'],
    ['onActivate', 'arguments']
  ])('should fail generation for %s %s', async (key, name) => {
    const spec = await demoNavSpec()
    spec.semantics = {
      ...spec.semantics,
      Logo: { role: 'action', label: 'Home', [key]: name }
    }
    expect(() => sourceFor(spec)).toThrow(
      `Invalid generated TypeScript identifier: ${JSON.stringify(name)}`
    )
  })
})
