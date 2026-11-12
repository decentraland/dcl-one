import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import ts from 'typescript'
import { afterEach, describe, expect, it } from 'vitest'
import type { IconSpec } from '../ir/schemas.js'
import { generateIcons } from './icons.js'

const context = {}

const icon = (overrides: Partial<IconSpec>): IconSpec => ({
  name: 'general/like',
  nodeId: '1:1',
  section: 'General',
  category: 'general',
  group: 'general',
  variant: null,
  width: 24,
  height: 24,
  file: 'x.svg',
  svg: '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M1 1h10v10H1z" stroke="#FCFCFC" stroke-width="2"/></svg>',
  ...overrides
})

// Declares and references ids through url(#), href and xlink:href.
const WITH_IDS =
  '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><defs><clipPath id="c"><path d="M0 0h12v12H0z"/></clipPath><path id="p" d="M1 1h4v4H1z"/></defs><mask id="m" fill="#fff"><path d="M2 2h20v20H2z"/></mask><g clip-path="url(#c)" mask="url(#m)"><path d="M0 0h24v24H0z" fill="#fff"/></g><use href="#p" fill="#fff"/><use xlink:href="#p" x="6" fill="#fff"/></svg>'

type Element = { tag: string; props: Record<string, unknown>; children: Element[] }
type Component = (props: Record<string, unknown>) => Element

const contentOf = (files: { path: string; content: string }[], file: string) =>
  files.find((candidate) => candidate.path === file)!.content

// Evaluates a generated category module with a minimal JSX factory and a React-like useId, so the
// rendered trees can be inspected without a DOM.
function loadCategory(source: string) {
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      jsx: ts.JsxEmit.React,
      jsxFactory: 'h'
    }
  })
  const calls = { useId: 0 }
  const react = { useId: () => `:r${calls.useId++}:` }
  const h = (tag: string, props: Record<string, unknown> | null, ...children: Element[]) => ({
    tag,
    props: props ?? {},
    children: children.flat()
  })
  const require = (name: string) => {
    if (name === 'react') return react
    throw new Error(`Unexpected import ${name}`)
  }
  const exports: Record<string, Component> = {}
  new Function('exports', 'require', 'h', outputText)(exports, require, h)
  return { components: exports, calls }
}

function walk(element: Element, visit: (element: Element) => void) {
  visit(element)
  element.children.forEach((child) => walk(child, visit))
}

function idsOf(root: Element): string[] {
  const ids: string[] = []
  walk(root, (element) => {
    if (typeof element.props.id === 'string') ids.push(element.props.id)
  })
  return ids
}

function referencesOf(root: Element): string[] {
  const references: string[] = []
  walk(root, (element) => {
    for (const [name, value] of Object.entries(element.props)) {
      if (typeof value !== 'string') continue
      for (const match of value.matchAll(/url\(#([^)]+)\)/g)) references.push(match[1]!)
      if ((name === 'href' || name === 'xlinkHref') && value.startsWith('#'))
        references.push(value.slice(1))
    }
  })
  return references
}

describe('generated icon ids', () => {
  describe('when the same icon renders twice', () => {
    const { files } = generateIcons([icon({ svg: WITH_IDS })], context)

    it('should give each instance disjoint ids', () => {
      const { components } = loadCategory(contentOf(files, 'icons/general.tsx'))
      const first = idsOf(components.GeneralLike!({}))
      const second = idsOf(components.GeneralLike!({}))

      expect(first.length).toBeGreaterThan(0)
      expect(first.filter((id) => second.includes(id))).toEqual([])
    })

    it('should resolve every url(#), href and xlink:href reference inside its own instance', () => {
      const { components } = loadCategory(contentOf(files, 'icons/general.tsx'))
      for (const root of [components.GeneralLike!({}), components.GeneralLike!({})]) {
        const ids = idsOf(root)
        const references = referencesOf(root)

        expect(references.length).toBeGreaterThanOrEqual(4)
        expect(references.every((reference) => ids.includes(reference))).toBe(true)
      }
    })

    it('should strip the useId separators so every id is a plain fragment identifier', () => {
      const { components } = loadCategory(contentOf(files, 'icons/general.tsx'))
      const ids = idsOf(components.GeneralLike!({}))

      expect(ids.every((id) => /^GeneralLike-r0-[\w-]+$/.test(id))).toBe(true)
    })

    it('should leave no id from the saved capture as a static string', () => {
      const source = contentOf(files, 'icons/general.tsx')

      expect(source).not.toMatch(/id="GeneralLike-/)
      expect(source).not.toMatch(/"#GeneralLike-|url\(#GeneralLike-/)
    })
  })

  describe('when an icon declares no ids', () => {
    it('should not call useId nor import it', () => {
      const { files } = generateIcons([icon({})], context)
      const source = contentOf(files, 'icons/general.tsx')
      const { components, calls } = loadCategory(source)

      components.GeneralLike!({})
      expect(source).not.toContain('useId')
      expect(calls.useId).toBe(0)
    })
  })

  describe('when an active-state icon declares ids', () => {
    const variants = [
      icon({ name: 'navbar/wallet', category: 'navbar', variant: 'active=false', svg: WITH_IDS }),
      icon({
        name: 'navbar/wallet',
        category: 'navbar',
        variant: 'active=true',
        nodeId: '1:2',
        svg: WITH_IDS.replaceAll('#fff', '#FF2D55')
      })
    ]

    it('should scope both branches with the ids of the rendering instance', () => {
      const { files } = generateIcons(variants, context)
      const { components } = loadCategory(contentOf(files, 'icons/navbar.tsx'))
      const active = components.NavbarWallet!({ active: true })
      const idle = components.NavbarWallet!({ active: false })

      expect(JSON.stringify(active)).toContain('#ff2d55')
      expect(JSON.stringify(idle)).not.toContain('#ff2d55')
      expect(idsOf(active).every((id) => id.startsWith('NavbarWallet-r0-'))).toBe(true)
      expect(idsOf(idle).every((id) => id.startsWith('NavbarWallet-r1-'))).toBe(true)
      for (const root of [active, idle])
        expect(referencesOf(root).every((reference) => idsOf(root).includes(reference))).toBe(true)
    })

    it('should call useId once per instance whichever branch renders', () => {
      const { files } = generateIcons(variants, context)
      const { components, calls } = loadCategory(contentOf(files, 'icons/navbar.tsx'))

      components.NavbarWallet!({ active: true })
      components.NavbarWallet!({})
      expect(calls.useId).toBe(2)
    })
  })
})

// A minimal React surface: no index signature on SVG props, so an unknown prop is a real error.
const REACT_STUB = `declare namespace JSX {
  interface Element { readonly element: true }
  interface IntrinsicElements { [tag: string]: Record<string, unknown> }
}
declare module 'react' {
  export interface SVGProps<T> { id?: string; className?: string; fill?: string; onClick?: (target: T) => void }
  export type ComponentType<P> = (props: P) => JSX.Element | null
  export function useId(): string
}
`

describe('generated icon registry typing', () => {
  let directory: string | undefined
  afterEach(async () => {
    if (directory) await rm(directory, { recursive: true, force: true })
    directory = undefined
  })

  async function typecheck(consumer: string): Promise<string[]> {
    directory = await mkdtemp(path.join(os.tmpdir(), 'icon-registry-'))
    const { files } = generateIcons(
      [
        icon({}),
        icon({ name: 'navbar/wallet', category: 'navbar', variant: 'active=false', nodeId: '1:2' }),
        icon({ name: 'navbar/wallet', category: 'navbar', variant: 'active=true', nodeId: '1:3' })
      ],
      context
    )
    await mkdir(path.join(directory, 'icons'))
    for (const file of files) await writeFile(path.join(directory, file.path), file.content)
    await writeFile(path.join(directory, 'react.d.ts'), REACT_STUB)
    await writeFile(path.join(directory, 'consumer.tsx'), consumer)
    const program = ts.createProgram(
      ['react.d.ts', 'consumer.tsx'].map((file) => path.join(directory!, file)),
      {
        strict: true,
        noUncheckedIndexedAccess: true,
        noEmit: true,
        jsx: ts.JsxEmit.Preserve,
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        target: ts.ScriptTarget.ES2022,
        lib: ['lib.es2022.d.ts', 'lib.dom.d.ts'],
        types: []
      }
    )
    return ts
      .getPreEmitDiagnostics(program)
      .map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))
  }

  const IMPORTS = `import type { ComponentType } from 'react'
import { activeIconComponents, iconComponents, iconMeta, isActiveIconComponentName, isIconComponentName, type IconComponentName, type IconProps } from './icons/index'
`

  it('should accept known keys, active on an active icon and lookups that are never undefined', async () => {
    const diagnostics = await typecheck(`${IMPORTS}
export const like = <iconComponents.GeneralLike size={24} />
export const wallet = <iconComponents.NavbarWallet active />
export const activeWallet = <activeIconComponents.NavbarWallet active={false} />
export const byName = (name: IconComponentName): ComponentType<IconProps> => iconComponents[name]
export const fromMeta: ComponentType<IconProps>[] = iconMeta.map((meta) => iconComponents[meta.component])
export const byString = (name: string) =>
  isActiveIconComponentName(name) ? activeIconComponents[name] : isIconComponentName(name) ? iconComponents[name] : null
`)

    expect(diagnostics).toEqual([])
  })

  it('should reject an unknown key and active on a non-active icon', async () => {
    const diagnostics = await typecheck(`${IMPORTS}
// @ts-expect-error GeneralUnknown is not a generated icon
export const unknown = iconComponents.GeneralUnknown
// @ts-expect-error GeneralLike has no active state
export const inactive = <iconComponents.GeneralLike active />
// @ts-expect-error GeneralLike is not an active-state icon
export const notActive = activeIconComponents.GeneralLike
// @ts-expect-error a plain string is not a registry key without the guard
export const unchecked = (name: string) => iconComponents[name]
`)

    expect(diagnostics).toEqual([])
  })

  it('should report the errors when they are not expected, so the check above is not vacuous', async () => {
    const diagnostics = await typecheck(`${IMPORTS}
export const unknown = iconComponents.GeneralUnknown
export const inactive = <iconComponents.GeneralLike active />
`)

    expect(diagnostics).toHaveLength(2)
    expect(diagnostics[0]).toContain('GeneralUnknown')
    expect(diagnostics[1]).toContain('active')
  })
})
