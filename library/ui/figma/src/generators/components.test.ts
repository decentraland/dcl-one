import { describe, expect, it } from 'vitest'
import { componentSpecs } from '../../test/snapshot.js'
import { externalizeSvgs, generateComponents } from './components.js'
import type { ViewNode } from '../ir/components.js'

const context = {}

async function generated() {
  return generateComponents(await componentSpecs(), context)
}

const fileContent = async (path: string) =>
  (await generated()).files.find((file) => file.path === path)!.content

describe('generateComponents', () => {
  describe('when generating the Button', () => {
    it('should emit a typed component that registers itself and defaults to the primary view', async () => {
      const source = await fileContent('components/buttons/Button.tsx')

      expect(source).toContain("variant?: 'primary' | 'secondary' | 'quaternary' | 'tertiary'")
      expect(source).toContain("variant = 'primary'")
      expect(source).toContain('attributes={rest}')
      expect(source).not.toContain('{...rest}')
      expect(source).toContain("registerComponent('Button', Button)")
      expect(source).toContain("import styles from './Button.module.css'")
      expect(source).toContain('figma-module-runtime')
      expect(source).not.toContain('styled-components')
    })

    it('should reference tokens and text styles instead of raw colours', async () => {
      const source = await fileContent('components/buttons/Button.module.css')

      expect(source).toContain('background: var(--dcl-primary-primary);')
      expect(
        source.startsWith(
          '@layer dcl.reset, dcl.foundations, dcl.components;\n\n@layer dcl.components {\n'
        )
      ).toBe(true)
      expect(source.trimEnd().endsWith('}')).toBe(true)
      expect(source).toContain("from '../../tokens/text-styles.module.css'")
      expect(source).not.toMatch(/#[0-9a-f]{6}/i)
    })

    it('should emit hover rules that work with real hover and with the forced state used by stories', async () => {
      const source = await fileContent('components/buttons/Button.module.css')

      expect(source).toContain(
        "&:hover:not(:disabled):not([data-interaction-mode='controlled']), &[data-force-state='hover'] {"
      )
    })
  })

  describe('when importing React types', () => {
    it('should import ReactNode only in components that declare slots', async () => {
      const sources = (await generated()).files.filter((file) => file.path.endsWith('.tsx'))
      const withSlots = sources.filter((file) => /\?: ReactNode/.test(file.content))

      expect(withSlots.length).toBeGreaterThan(0)
      expect(withSlots.length).toBeLessThan(sources.length)
      for (const file of sources)
        expect(file.content.includes(', ReactNode }')).toBe(withSlots.includes(file))
    })
  })

  describe('when a component embeds another one', () => {
    it('should import it from its own page folder so it is registered', async () => {
      const source = await fileContent('components/controllers/SwitchTitle.tsx')

      expect(source).toContain("import '../buttons/Switch'")
    })
  })

  describe('when listing what needs manual work', () => {
    it('should expose every reason in the manifest and the barrel file', async () => {
      const manifest = await fileContent('components/manifest.ts')
      const index = await fileContent('components/index.ts')

      expect(manifest).toContain('"unresolved-instance:ContextualMenuIcons"')
      expect(index).toContain("export * from './buttons/Button'")
      expect(index).toContain("export * from './manifest'")
    })
  })

  describe('when generating twice', () => {
    it('should produce byte-identical files', async () => {
      const first = await generateComponents(await componentSpecs(), context)
      const second = await generateComponents(await componentSpecs(), context)

      expect(second.files).toEqual(first.files)
    })
  })

  describe('when a stylesheet embeds a large SVG', () => {
    const svgUrl = (svg: string) => `url("data:image/svg+xml,${encodeURIComponent(svg)}")`
    const large = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,${'A'.repeat(5000)}"/></svg>`

    it('should move it to a content-addressed asset referenced once per use', () => {
      const assets = new Map<string, string>()
      const css = externalizeSvgs(
        `.a { background: ${svgUrl(large)}; }\n.b { background: ${svgUrl(large)}; }\n`,
        assets
      )

      expect(assets.size).toBe(1)
      const [name, content] = [...assets][0]!
      expect(content).toBe(large)
      expect(css).toBe(
        `.a { background: url("../assets/${name}"); }\n.b { background: url("../assets/${name}"); }\n`
      )
    })

    it.each([
      ['a script', '<script>alert(1)</script>'],
      ['an event handler', '<rect onload="alert(1)"/>'],
      ['an external link', '<image href="https://example.com/a.png"/>'],
      ['a javascript link', '<a xlink:href="javascript:alert(1)"/>'],
      ['an animation', '<set attributeName="href" to="javascript:alert(1)"/>'],
      ['an entity declaration', '<!ENTITY x "y">'],
      ['a namespaced script', '<x:script xmlns:x="http://www.w3.org/2000/svg">alert(1)</x:script>'],
      ['a foreign namespace', '<g xmlns="http://www.w3.org/1999/xhtml"/>'],
      ['an unknown attribute', '<rect src="https://example.com/a"/>'],
      ['an external paint reference', '<rect fill="url(https://example.com/a.svg#p)"/>'],
      ['text content', '<g>alert(1)</g>'],
      ['an encoded external reference', '<rect fill="u&#114;l(https://example.com/a.svg#p)"/>'],
      ['an escaped external reference', '<rect fill="u\\72 l(https://example.com/a.svg#p)"/>'],
      ['an uppercase external reference', '<rect fill="URL (https://example.com/a.svg#p)"/>'],
      ['an external filter', '<g filter="url(https://example.com/a.svg#f)"/>'],
      ['a filter on a missing id', '<g filter="url(#missing)"/>'],
      ['an escaped filter', '<g filter="url(#f\\30)"/>'],
      ['a style beyond blend mode and mask type', '<g style="mix-blend-mode:overlay;color:red"/>'],
      ['a style with a reference', '<g style="background:url(#f0)"/>'],
      ['a non-numeric composite coefficient', '<feComposite k2="1px"/>'],
      ['an unknown rendering hint', '<rect shape-rendering="pixelated"/>'],
      ['a filter on an element that is not a filter', '<rect id="r0"/><g filter="url(#r0)"/>'],
      ['a filter on an id defined twice', '<filter id="d"/><g id="d"/><g filter="url(#d)"/>'],
      ['a repeated style declaration', '<g style="mix-blend-mode:overlay;mix-blend-mode:screen"/>'],
      ['an empty style', '<g style=""/>'],
      ['a composite coefficient outside feComposite', '<rect k2="1"/>']
    ])('should refuse to publish one containing %s', (_, active) => {
      const svg = large.replace('</svg>', `${active}</svg>`)

      expect(() => externalizeSvgs(`.a { background: ${svgUrl(svg)}; }`, new Map())).toThrow(
        'Refusing to publish an SVG asset:'
      )
    })

    it('should publish the shadows, blend modes and masks Figma exports', () => {
      const figma = large.replace(
        '</svg>',
        '<g style="mix-blend-mode:overlay" filter="url(#filter0_i_1_2)"><rect shape-rendering="crispEdges"/></g>' +
          '<mask id="m" style="mask-type:alpha"/><defs><filter id="filter0_i_1_2" filterUnits="userSpaceOnUse">' +
          '<feComposite in2="hardAlpha" operator="arithmetic" k2="-1" k3="1"/></filter></defs></svg>'
      )
      const assets = new Map<string, string>()

      externalizeSvgs(`.a { background: ${svgUrl(figma)}; }`, assets)

      expect([...assets.values()]).toEqual([figma])
    })

    it('should keep a small SVG inline', () => {
      const assets = new Map<string, string>()
      const css = `.a { background: ${svgUrl('<svg xmlns="http://www.w3.org/2000/svg"/>')}; }`

      expect(externalizeSvgs(css, assets)).toBe(css)
      expect(assets.size).toBe(0)
    })
  })
})

describe('generateComponents with a Figma name that holds a line terminator', () => {
  it.each([
    ['a line feed', '\n'],
    ['a carriage return', '\r'],
    ['a line separator', '\u2028'],
    ['a paragraph separator', '\u2029']
  ])('should keep %s inside the header comment instead of emitting code', async (_, terminator) => {
    const specs = await componentSpecs()
    const target = specs[0]!
    target.figmaName = `Button${terminator}export const injected = 1`
    target.page = `Page${terminator}globalThis.pwned = true`
    const source = generateComponents(specs, context).files.find((file) =>
      file.path.endsWith(`/${target.name}.tsx`)
    )!.content

    const lines = source.split(/\r\n|[\n\r\u2028\u2029]/)
    expect(lines.some((line) => /^\s*(export const injected|globalThis\.pwned)/.test(line))).toBe(
      false
    )
    expect(lines.filter((line) => line.startsWith('// Figma: '))).toEqual([
      expect.stringContaining('Page globalThis.pwned = true / Button export const injected = 1')
    ])
  })
})

describe('generateComponents with opt-in semantics', () => {
  const boxes = (node: ViewNode): ViewNode[] => [
    ...(node.kind === 'box' && node.path.length > 0 ? [node] : []),
    ...(node.children ?? []).flatMap(boxes)
  ]
  // The first component with a nested box gets a navigation root and one action.
  const withSemantics = async () => {
    const specs = await componentSpecs()
    const target = specs.find((spec) => spec.views.some((view) => boxes(view.root).length > 0))!
    const box = target.views.flatMap((view) => boxes(view.root))[0]!
    target.semantics = {
      '@root': { role: 'navigation', label: 'Main' },
      [box.name]: { role: 'action', href: 'baseHref', onActivate: 'onBase' }
    }
    return { specs, target }
  }

  describe('when only one component declares semantics', () => {
    it('should leave every other generated file byte-identical', async () => {
      const plain = await generated()
      const { specs, target } = await withSemantics()
      const semantic = generateComponents(specs, context)

      const changed = semantic.files
        .filter(
          (file) => plain.files.find((other) => other.path === file.path)?.content !== file.content
        )
        .map((file) => file.path)
      expect(changed.every((path) => path.endsWith(`/${target.name}.tsx`))).toBe(true)
    })
  })

  describe('when a component declares semantics', () => {
    it('should emit the semantic nodes and the props they name, passed to FigmaView', async () => {
      const { specs, target } = await withSemantics()
      const source = generateComponents(specs, context).files.find((file) =>
        file.path.endsWith(`/${target.name}.tsx`)
      )!.content

      expect(source).toContain('semantic: {"role":"navigation","label":"Main"}')
      expect(source).toContain('  baseHref?: string')
      expect(source).toContain('  onBase?: () => void')
      expect(source).toContain('semanticProps={{ baseHref, onBase }}')
    })
  })
})

describe('generateComponents with Figma motion', () => {
  it('should transition what the hover changes and fade a layer the hover reveals', async () => {
    const spec = structuredClone(
      (await componentSpecs()).find((entry) => entry.name === 'SquaredButton')!
    )
    const view = spec.views[0]!
    view.motion = { hover: { duration: 250, easing: 'ease-in-out' } }
    const revealed = view.hover!.find((rule) =>
      rule.decls.some(([property, value]) => property === 'display' && value !== 'none')
    )!
    const key = revealed.path.join('.')
    const css = generateComponents([spec], context).files.find((file) =>
      file.path.endsWith('SquaredButton.module.css')
    )!.content

    expect(css).toContain(
      `& [data-n='${key}'] {\n  opacity: 0;\n  transition-property: display, opacity;`
    )
    expect(css).toContain(
      '  transition-duration: 250ms;\n  transition-timing-function: ease-in-out;\n  transition-behavior: allow-discrete;'
    )
    expect(css).toMatch(
      new RegExp(
        `\\[data-force-state='hover'\\] \\[data-n='${key.replace('.', '\\.')}'\\] \\{\\n  display: [a-z]+;\\n  opacity: 1;\\n  @starting-style \\{\\n    opacity: 0;\\n  \\}`
      )
    )
    expect(css).toContain('@media (prefers-reduced-motion: reduce) {')
  })

  it('should transition a gradient ring on the pseudo-element itself, with reduced motion', async () => {
    const spec = structuredClone(
      (await componentSpecs()).find((entry) => entry.name === 'SquaredButton')!
    )
    const view = spec.views[0]!
    // A ring present at rest, hidden, and faded in by the hover, as the view mapper persists it.
    view.root.css.after = [
      ['content', "''"],
      ['opacity', '0']
    ]
    view.hover = [...(view.hover ?? []), { path: [], decls: [], after: [['opacity', '1']] }]
    view.motion = { hover: { duration: 300, easing: 'ease-out' } }
    const css = generateComponents([spec], context).files.find((file) =>
      file.path.endsWith('SquaredButton.module.css')
    )!.content

    expect(css).toContain("&::after {\n  content: '';\n  opacity: 0;\n}")
    expect(css).toContain(
      '&::after {\n  transition-property: opacity;\n  transition-duration: 300ms;\n  transition-timing-function: ease-out;\n}'
    )
    expect(css).toContain(
      "&:hover:not(:disabled):not([data-interaction-mode='controlled'])::after, &[data-force-state='hover']::after {\n  opacity: 1;\n}"
    )
    expect(css).toContain(
      '@media (prefers-reduced-motion: reduce) {\n  &, & [data-n], &::after, & [data-n]::after {\n    transition: none;'
    )
  })

  it('should not give the element an empty transition when a state only changes its ring', async () => {
    const spec = structuredClone(
      (await componentSpecs()).find((entry) => entry.name === 'SquaredButton')!
    )
    const view = spec.views[0]!
    view.root.css.after = [['opacity', '0']]
    view.hover = [{ path: [], decls: [], after: [['opacity', '1']] }]
    view.motion = { hover: { duration: 300, easing: 'ease-out' } }
    const css = generateComponents([spec], context).files.find((file) =>
      file.path.endsWith('SquaredButton.module.css')
    )!.content

    expect(css).not.toContain('transition-property: ;')
    expect(css).toContain('&::after {\n  transition-property: opacity;')
  })

  it('should emit no transition for a view without motion', async () => {
    const css = await fileContent('components/buttons/Button.module.css')

    expect(css).not.toContain('transition')
  })

  describe('when the default selection of a component is not a captured view', () => {
    it('should fail generation instead of emitting a component that throws when rendered with no props', async () => {
      const spec = structuredClone((await componentSpecs()).find((item) => item.name === 'Button')!)
      const variant = spec.props.find((prop) => prop.name === 'variant')!
      spec.views = spec.views.filter((view) => view.values.variant !== variant.default)
      expect(() => generateComponents([spec], context)).toThrow(
        `Button: its default selection (variant="${String(variant.default)}"`
      )
    })
  })
})
