import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { planComponents, type ComponentPlanContext } from './install-components.js'
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
      { components: [] },
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
            .node, .node:hover, .active::before {
              composes: shared from "./shared.module.css";
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

  it.each([[], ['DemoNav', 'DemoNav'], ['../DemoNav'], ['3D Card'], ['Missing']])(
    'should reject invalid or unavailable registry names %j',
    async (...components) => {
      context.registry = { components }
      await expect(planComponents(context)).rejects.toThrow()
    }
  )

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
