import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { preprocessCSS, resolveConfig, type ResolvedConfig } from 'vite'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { planComponents, cssDeclarations, type ComponentPlanContext } from './install-components.js'
import { runtimeFiles } from './promotion/eligibility.js'
import * as eligibility from './promotion/eligibility.js'
import * as specs from './specs.js'
import { parseMappingSource } from './contract/mapping-file.js'
import { snapshotSource, testConfig } from '../test/snapshot.js'

describe('when planning public components from verified installer inputs', () => {
  let context: ComponentPlanContext

  beforeEach(async () => {
    context = {
      source: snapshotSource(),
      config: { ...(await testConfig()), imageFills: 'placeholder' },
      registry: { components: ['DemoNav'] },
      runtime: new Map(
        runtimeFiles.map((file) => [
          file,
          Buffer.from(
            file.endsWith('.css')
              ? '.node { display: block }'
              : `export * from '../generated/icons'`
          )
        ])
      )
    }
  })

  afterEach(() => vi.restoreAllMocks())

  it('should return deterministic public files without changing the inputs', async () => {
    const before = [...context.runtime].map(([file, bytes]) => [file, bytes.toString()])
    const first = await planComponents(context)
    const second = await planComponents(context)
    expect(first.components).toEqual(['DemoNav'])
    expect(second).toEqual(first)
    expect([...context.runtime].map(([file, bytes]) => [file, bytes.toString()])).toEqual(before)
    expect(JSON.parse(first.files.get('promotion.json')!.toString())).toEqual({
      components: ['DemoNav']
    })
    expect(first.files.has('DemoNav/DemoNav.tsx')).toBe(true)
    expect(first.files.has('DemoNav/DemoNav.module.css.d.ts')).toBe(true)
    expect(first.files.has('DemoNav/DemoNav.figma.ts')).toBe(true)
    expect(first.files.has('runtime/figma-module-runtime.module.css.d.ts')).toBe(true)
    expect(
      [...first.files.keys()].some(
        (file) => file.includes('snapshot') || file.includes('reference')
      )
    ).toBe(false)
  })

  it.each(
    [
      null,
      undefined,
      [],
      'DemoNav',
      { internal: ['DemoNav'] },
      { components: ['DemoNav'], internal: null },
      { components: ['DemoNav'], internal: [3] }
    ].map((registry) => ({ registry }))
  )(
    'should reject malformed registry containers before reading component input: %j',
    async ({ registry }) => {
      const read = vi.spyOn(context.source, 'getComponentPage')
      context.registry = registry as unknown as ComponentPlanContext['registry']
      await expect(planComponents(context)).rejects.toThrow('Component registry')
      expect(read).not.toHaveBeenCalled()
    }
  )

  describe.each([{ components: [] }, { components: [], internal: [] }])(
    'and the registry is legally empty: %j',
    (registry) => {
      beforeEach(() => {
        context.registry = registry
        context.source = new Proxy(context.source, {
          get() {
            throw new Error('Empty plans must not read source inputs')
          }
        })
        context.runtime = new Proxy(context.runtime, {
          get() {
            throw new Error('Empty plans must not read runtime inputs')
          }
        })
      })
      it('should deterministically return only the normalized empty registry', async () => {
        const first = await planComponents(context)
        expect(first).toEqual({
          components: [],
          files: new Map([['promotion.json', Buffer.from('{\n  "components": []\n}\n')]])
        })
        expect(await planComponents(context)).toEqual(first)
      })
      it('should still enforce the configured raster policy before returning an empty plan', async () => {
        context.config.imageFills = 'assets'
        await expect(planComponents(context)).rejects.toThrow(
          'Public component installation requires the placeholder raster policy'
        )
      })
    }
  )

  describe('and the registry declares internal visibility', () => {
    beforeEach(() => {
      context.registry = { components: [], internal: ['DemoNav'] }
    })
    it('should install internal output without advertising a mapping or adding a public entry', async () => {
      const plan = await planComponents(context)
      expect(plan.files.has('DemoNav/DemoNav.tsx')).toBe(true)
      expect(plan.files.has('DemoNav/DemoNav.figma.ts')).toBe(false)
      expect(plan.files.has('index.ts')).toBe(false)
      expect(JSON.parse(plan.files.get('promotion.json')!.toString())).toEqual({
        components: [],
        internal: ['DemoNav']
      })
    })
    it('should reject a component listed with both public and internal visibility', async () => {
      context.registry = { components: ['DemoNav'], internal: ['DemoNav'] }
      await expect(planComponents(context)).rejects.toThrow('unique component names')
    })
    it('should reject an unknown internal component', async () => {
      context.registry = { components: [], internal: ['Missing'] }
      await expect(planComponents(context)).rejects.toThrow('Unknown component')
    })
    it('should reject malformed internal names instead of omitting them', async () => {
      context.registry = { components: [], internal: 'DemoNav' as unknown as string[] }
      await expect(planComponents(context)).rejects.toThrow('public and internal name lists')
    })
  })

  it('should relocate imports and preserve the generated sparse and suppressed-prop contracts', async () => {
    const plan = await planComponents(context)
    const component = plan.files.get('DemoNav/DemoNav.tsx')!.toString()
    expect(component).toContain(`from '../runtime/figma-module-runtime'`)
    expect(component).not.toContain('../../../components/')
    expect(component).toContain('attributes={rest}')
    expect(component).toContain('suppressed={demoNavSuppressedProps}')
    expect(plan.files.get('runtime/figma-module-runtime.tsx')!.toString()).toContain(
      `from '../../foundations/icons'`
    )
    expect(plan.files.get('DemoNav/DemoNav.figma.ts')!.toString()).toContain('url=<DCL_FIGMA_FILE>')
  })

  describe('and runtime styles contain selectors alongside dotted non-selector content', () => {
    beforeEach(() => {
      context.runtime = new Map(context.runtime).set(
        'figma-module-runtime.module.css',
        Buffer.from(`
          @layer reset, foundations.typography, components.controls;
          @import url("./base.module.css");
          /* .commented-out { color: red } */
          @layer components.controls {
            .node { composes: shared from global; }
            .node, .node:hover, .active::before {
              background: url("data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><path class='.fake' /></svg>");
              mask: url(https://example.org/icon.svg);
              content: ".quoted \\".escaped";
            }
            @media (min-width: 1.5px) {
              .node:is(.selected, .focused):not(.disabled)[data-kind=".attribute"] > .child {
                background-image: url('./assets/icon.svg');
              }
            }
            @supports selector(.unsupported) {
              .supported:where(.enabled) { color: red }
            }
          }
        `)
      )
    })

    it('should emit only distinct classes from qualified selectors', async () => {
      expect(
        (await planComponents(context)).files
          .get('runtime/figma-module-runtime.module.css.d.ts')!
          .toString()
      ).toBe(
        'declare const styles: {\n' +
          ['node', 'active', 'selected', 'focused', 'disabled', 'child', 'supported', 'enabled']
            .map((name) => '  readonly "' + name + '": string')
            .join('\n') +
          '\n}\nexport default styles\n'
      )
    })
  })

  describe('and runtime styles contain no class selectors', () => {
    beforeEach(() => {
      context.runtime = new Map(context.runtime).set(
        'figma-module-runtime.module.css',
        Buffer.from(
          '@layer reset, foundations.typography; :root { --asset: url(https://example.org/icon.svg); } /* .comment */'
        )
      )
    })
    it('should emit an empty declaration without inventing layer or URL exports', async () => {
      expect(
        (await planComponents(context)).files
          .get('runtime/figma-module-runtime.module.css.d.ts')!
          .toString()
      ).toBe('declare const styles: {\n\n}\nexport default styles\n')
    })
  })

  it('should emit a mapping accepted by the existing literal parser', async () => {
    const plan = await planComponents(context)
    const mapping = parseMappingSource(
      plan.files.get('DemoNav/DemoNav.figma.ts')!.toString(),
      'DemoNav.figma.ts'
    )
    expect(mapping.issues).toEqual([])
  })

  it.each([
    {
      components: ['DemoNav', 'DemoNav'],
      error: 'Component registry must contain unique component names only'
    },
    { components: ['../DemoNav'], error: 'Invalid generated TypeScript identifier' },
    { components: ['3D Card'], error: 'Invalid generated TypeScript identifier' },
    { components: ['Missing'], error: 'Unknown component or missing dependency: Missing' },
    {
      components: [
        {
          name: 'DemoNav',
          limits: [
            { id: 'a', text: 'x.' },
            { id: 'a', text: 'y.' }
          ]
        }
      ],
      error: 'Invalid or duplicate accepted limit'
    }
  ])(
    'should reject invalid or unavailable registry names $components',
    async ({ components, error }) => {
      context.registry = { components }
      await expect(planComponents(context)).rejects.toThrow(error)
    }
  )

  it('should write the accepted limits back with the public name', async () => {
    const limits = [{ id: 'logo-size', text: 'The logo is pending.' }]
    context.registry = { components: [{ name: 'DemoNav', limits }] }

    const plan = await planComponents(context)

    expect(JSON.parse(plan.files.get('promotion.json')!.toString())).toEqual({
      components: [{ name: 'DemoNav', limits }]
    })
  })

  it('should refuse limits on an internal name', async () => {
    context.registry = {
      components: [],
      internal: [{ name: 'DemoNav', limits: [{ id: 'a', text: 'x.' }] }] as unknown as string[]
    }
    await expect(planComponents(context)).rejects.toThrow('names only, without limits')
  })

  it('should reject a legacy seal in place of the names-only registry', async () => {
    context.registry = {
      components: ['DemoNav'],
      files: {}
    } as ComponentPlanContext['registry']
    await expect(planComponents(context)).rejects.toThrow('public and internal name lists only')
  })

  it('should consume a live source supplied by the core without selecting a fallback', async () => {
    const input = context.source
    context.source = new Proxy(input, {
      get(target, key) {
        if (key === 'name') return 'rest'
        const value = Reflect.get(target, key, target)
        return typeof value === 'function' ? value.bind(target) : value
      }
    })
    const plan = await planComponents(context)
    expect(plan.components).toEqual(['DemoNav'])
  })

  it('should reject missing live component data rather than returning an empty plan', async () => {
    const input = context.source
    context.source = new Proxy(input, {
      get(target, key) {
        if (key === 'name') return 'rest'
        if (key === 'getComponentPageById') return async () => undefined
        const value = Reflect.get(target, key, target)
        return typeof value === 'function' ? value.bind(target) : value
      }
    })
    await expect(planComponents(context)).rejects.toThrow()
  })

  it('should refuse raster-asset generation instead of reintroducing captured mock images', async () => {
    context.config.imageFills = 'assets'
    await expect(planComponents(context)).rejects.toThrow('placeholder raster policy')
  })

  it.each(runtimeFiles)('should refuse a missing runtime input %s', async (file) => {
    context.runtime = new Map([...context.runtime].filter(([name]) => name !== file))
    await expect(planComponents(context)).rejects.toThrow('Missing locked runtime input: ' + file)
  })

  it('should reject components excluded by the source eligibility policy', async () => {
    vi.spyOn(eligibility, 'promotionExclusion').mockReturnValue(['manual review required'])
    await expect(planComponents(context)).rejects.toThrow('Component is not eligible: DemoNav')
  })

  it.each(['MissingDependency', 'DemoNav'])(
    'should reject an unavailable or recursive component dependency %s',
    async (dependency) => {
      const loaded = structuredClone(
        await specs.loadSpecsFromSource(context.source, context.config, 'promotion')
      )
      loaded.specs.find((spec) => spec.name === 'DemoNav')!.views[0]!.root.ref = {
        component: dependency,
        props: {}
      }
      vi.spyOn(specs, 'loadSpecsFromSource').mockResolvedValueOnce(loaded)
      await expect(planComponents(context)).rejects.toThrow(
        dependency === 'DemoNav' ? 'Recursive component dependency' : 'missing dependency'
      )
    }
  )

  it('should reject a capture that disappears before mapping emission', async () => {
    const original = context.source.getComponentPageById!.bind(context.source)
    let navReads = 0
    vi.spyOn(context.source, 'getComponentPageById').mockImplementation(async (id, name) => {
      if (id === '10:7' && ++navReads === 2) return undefined
      return original(id, name)
    })
    await expect(planComponents(context)).rejects.toThrow('Missing captured component mapping')
  })

  it('should propagate source read failures without returning a partial plan', async () => {
    vi.spyOn(context.source, 'getTokenCollections').mockRejectedValueOnce(new Error('read failed'))
    await expect(planComponents(context)).rejects.toThrow('read failed')
  })
})

describe('when extracting CSS Module declarations through the in-memory stylesheet graph', () => {
  let files: Map<string, Buffer>
  let foundations: string
  let config: ResolvedConfig
  const keys = (declaration: Buffer) =>
    [...declaration.toString().matchAll(/readonly ("(?:[^"\\]|\\.)*"): string/g)]
      .map((match) => JSON.parse(match[1]!))
      .sort()
  beforeEach(async () => {
    files = new Map()
    foundations = '.body { color: red }'
    config = await resolveConfig({ configFile: false, css: { transformer: 'postcss' } }, 'build')
  })

  describe.each(['NavBar/NavBar.module.css', 'runtime/figma-module-runtime.module.css'])(
    'and reading the committed %s stylesheet',
    (file) => {
      let absolute: string
      let content: string
      beforeEach(async () => {
        absolute = fileURLToPath(new URL('../../components/' + file, import.meta.url))
        content = await readFile(absolute, 'utf8')
        files.set(file, Buffer.from(content))
        foundations = await readFile(
          new URL('../../foundations/typography/text-styles.module.css', import.meta.url),
          'utf8'
        )
      })
      it('should declare exactly the keys Vite exports', async () => {
        expect(keys((await cssDeclarations(files, foundations)).get(file + '.d.ts')!)).toEqual(
          Object.keys((await preprocessCSS(content, absolute, config)).modules ?? {}).sort()
        )
      })
    }
  )

  describe.each([
    ':global(.global) {} .local :global .outside {} :local(.explicit) {}',
    String.raw`.a\:hover {} .\31 23 {} .-dash {} .Ünï { color: red }`,
    '@keyframes spin { to { opacity: 1 } } .animated { animation: spin 1s }',
    '.k { grid-area: a ] b } .after {}'
  ])('and processing %s', (content) => {
    beforeEach(() => {
      files.set('test.module.css', Buffer.from(content))
    })
    it('should match Vite exports without declaration tokens hiding later classes', async () => {
      expect(
        keys((await cssDeclarations(files, foundations)).get('test.module.css.d.ts')!)
      ).toEqual(
        Object.keys(
          (await preprocessCSS(content, '/components/test.module.css', config)).modules ?? {}
        ).sort()
      )
    })
  })

  describe('and a component composes from the generated foundations and another planned stylesheet', () => {
    beforeEach(() => {
      files.set(
        'A/A.module.css',
        Buffer.from(
          '.root { composes: body from "../../foundations/typography/text-styles.module.css"; composes: shared from "../B/B.module.css"; }'
        )
      )
      files.set('B/B.module.css', Buffer.from('.shared { color: red }'))
    })
    it('should resolve real exported names without reading the filesystem', async () => {
      expect(keys((await cssDeclarations(files, foundations)).get('A/A.module.css.d.ts')!)).toEqual(
        ['root']
      )
    })
  })

  describe.each([
    [
      'missing relative stylesheet',
      '.root { composes: missing from "./missing.module.css" }',
      'Unknown CSS module input'
    ],
    [
      'package import',
      '.root { composes: missing from "some-package" }',
      'Unsupported CSS module import'
    ],
    [
      'self composition import',
      '.root { composes: root from "./test.module.css" }',
      'Recursive CSS module composition'
    ],
    ['unclosed block', '.root { color: red', 'Unclosed block'],
    ['invalid selector', '.root :global(.broken { color: red }', 'Unclosed bracket']
  ])('and encountering %s', (_label, content, message) => {
    beforeEach(() => {
      files.set('test.module.css', Buffer.from(content))
    })
    it('should reject without returning a partial declarations map', async () => {
      await expect(cssDeclarations(files, foundations)).rejects.toThrow(message)
    })
  })
})
