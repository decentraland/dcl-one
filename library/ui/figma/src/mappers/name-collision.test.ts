import { readFile, rm } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { runSync } from '../app.js'
import { configSchema } from '../config.js'
import { generateComponents } from '../generators/components.js'
import { GEN, SNAP, json, lifecycleFixture, put } from '../guard/lifecycle-fixture.js'
import { rawComponentPageSchema } from '../ir/raw-components.js'
import {
  buildComponentSpecsWithCollisions,
  collisionReason,
  type ComponentPageInput
} from './component-spec.js'

const component = (id: string, name: string) => ({
  kind: 'component',
  id,
  name,
  section: [],
  x: 0,
  y: 0,
  properties: {},
  base: { id, name, props: {}, tree: { type: 'FRAME', name, w: 20, h: 20 } },
  others: []
})
const page = (name: string, components: ReturnType<typeof component>[]): ComponentPageInput => ({
  name,
  category: 'Components',
  data: rawComponentPageSchema.parse({ page: { id: name, name }, components })
})
const config = (componentNames: Record<string, string> = {}) =>
  configSchema.parse({
    outDir: 'review/src/generated',
    pages: {},
    componentNames
  })
const build = (pages: ComponentPageInput[], aliases?: Record<string, string>) => {
  const result = buildComponentSpecsWithCollisions({
    pages,
    config: config(aliases),
    collections: [],
    textStyles: [],
    icons: new Map()
  })
  const files = generateComponents(result.specs, {}).files
  return { ...result, files }
}
const paths = (files: { path: string }[]) => files.map((file) => file.path).sort()
const index = (files: { path: string; content: string }[]) =>
  files.find((file) => file.path === 'components/index.ts')!.content

describe('when two components generate the same name', () => {
  let pages: ComponentPageInput[]
  let result: ReturnType<typeof build>

  beforeEach(() => {
    pages = [
      page('Buttons', [
        component('16326:4282', 'IconButton'),
        component('15785:43182', 'Icon Button'),
        component('10:1', 'Close Button')
      ])
    ]
  })

  describe('and no alias resolves the collision', () => {
    beforeEach(() => {
      result = build(pages)
    })

    it('should generate only the component that does not collide', () => {
      expect(result.specs.map((spec) => spec.name)).toEqual(['CloseButton'])
    })

    it('should write no file for the colliding name', () => {
      expect(paths(result.files).filter((file) => /IconButton/i.test(file))).toEqual([])
    })

    it('should not export the colliding name from the index', () => {
      expect(index(result.files)).not.toMatch(/IconButton/)
    })

    it('should report one sorted diagnostic', () => {
      expect(result.collisions.map(collisionReason)).toEqual([
        'name-collision:IconButton: 15785:43182 "Icon Button", 16326:4282 "IconButton"'
      ])
    })

    describe('and the capture order is reversed', () => {
      let backward: ReturnType<typeof build>

      beforeEach(() => {
        backward = build([page('Buttons', [...pages[0]!.data.components].reverse() as never)])
      })

      it('should produce byte-identical output', () => {
        expect(backward.files.map((file) => [file.path, file.content]).sort()).toEqual(
          result.files.map((file) => [file.path, file.content]).sort()
        )
      })

      it('should produce identical diagnostics', () => {
        expect(backward.collisions).toEqual(result.collisions)
      })
    })
  })

  describe('and distinct aliases resolve the collision', () => {
    beforeEach(() => {
      result = build(pages, {
        'Icon Button': 'IconButtonOutlined',
        IconButton: 'IconButtonFilled'
      })
    })

    it('should generate both under their aliases', () => {
      expect(result.specs.map((spec) => [spec.figmaId, spec.name])).toEqual([
        ['16326:4282', 'IconButtonFilled'],
        ['15785:43182', 'IconButtonOutlined'],
        ['10:1', 'CloseButton']
      ])
    })

    it('should report no collision', () => {
      expect(result.collisions).toEqual([])
    })
  })

  describe('and the names differ only in case', () => {
    beforeEach(() => {
      result = build([page('A', [component('1:1', 'Iconbutton'), component('1:2', 'IconButton')])])
    })

    it('should generate neither', () => {
      expect(result.specs).toEqual([])
    })

    it('should report them as a collision', () => {
      expect(result.collisions).toHaveLength(1)
    })
  })
})

describe('when names do not collide', () => {
  let result: ReturnType<typeof build>

  beforeEach(() => {
    result = build([
      page('Buttons', [component('1:1', 'Close Button'), component('1:2', 'Button-Plain')])
    ])
  })

  it('should keep every generated name unchanged', () => {
    expect(result.specs.map((spec) => spec.name)).toEqual(['CloseButton', 'ButtonPlain'])
  })

  it('should report no collision', () => {
    expect(result.collisions).toEqual([])
  })
})

describe('when the same Figma id is captured on two pages', () => {
  let result: ReturnType<typeof build>

  describe('and both captures carry the same name', () => {
    beforeEach(() => {
      const shared = component('5:5', 'NavMenuTab')
      result = build([page('Nav bar', [shared]), page('Bars legacy', [shared])])
    })

    it('should generate it once, from its first configured page', () => {
      expect(result.specs.map((spec) => [spec.name, spec.page])).toEqual([
        ['NavMenuTab', 'Nav bar']
      ])
    })

    it('should write its file only in the first page folder', () => {
      expect(paths(result.files).filter((file) => file.endsWith('NavMenuTab.tsx'))).toEqual([
        'components/nav-bar/NavMenuTab.tsx'
      ])
    })

    it('should export it once from the index', () => {
      expect(index(result.files).match(/NavMenuTab'/g)).toHaveLength(1)
    })

    it('should report no diagnostic', () => {
      expect(result.collisions).toEqual([])
    })
  })

  describe.each([
    ['different raw names', {}],
    ['different aliases', { 'Nav Menu Tab': 'NavTabPrimary', 'Legacy Tab': 'NavTabLegacy' }]
  ])('and the captures carry %s', (_, aliases) => {
    beforeEach(() => {
      result = build(
        [
          page('Nav bar', [component('5:5', 'Nav Menu Tab')]),
          page('Bars legacy', [component('5:5', 'Legacy Tab')])
        ],
        aliases
      )
    })

    it('should generate it once, from its first configured page', () => {
      expect(result.specs.map((spec) => [spec.figmaId, spec.page])).toEqual([['5:5', 'Nav bar']])
    })

    it('should write its file only in the first page folder', () => {
      const name = result.specs[0]!.name
      expect(paths(result.files).filter((file) => file.endsWith('.tsx'))).toEqual([
        `components/nav-bar/${name}.tsx`
      ])
    })

    it('should export only the first page folder from the index', () => {
      expect(index(result.files).match(/^export \* from '\.\/[a-z-]+\//gm)).toEqual([
        "export * from './nav-bar/"
      ])
    })

    it('should report no diagnostic', () => {
      expect(result.collisions).toEqual([])
    })
  })

  describe('and another id takes the same name', () => {
    beforeEach(() => {
      const shared = component('9141:1615', 'NavBar')
      result = build([
        page('Chat', [shared]),
        page('Bars', [component('15599:30721', 'NavBar')]),
        page('Legacy', [shared])
      ])
    })

    it('should generate neither', () => {
      expect(result.specs).toEqual([])
    })

    it('should fail closed, listing distinct ids', () => {
      expect(result.collisions.map(collisionReason)).toEqual([
        'name-collision:NavBar: 9141:1615 "NavBar", 15599:30721 "NavBar"'
      ])
    })
  })
})

describe('when a previously generated component starts colliding', () => {
  let root: string
  let app: Awaited<ReturnType<typeof lifecycleFixture>>['app']
  let generated: string

  beforeEach(async () => {
    ;({ root, app } = await lifecycleFixture())
    const snapshot = await json(root, `${SNAP}/components/Page.json`)
    snapshot.components[1].name = 'First'
    snapshot.components[1].base.name = 'First'
    await put(root, `${SNAP}/components/Page.json`, snapshot)
    generated = await readFile(path.join(root, GEN, 'page/First.tsx'), 'utf8')
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  it('should block the removal instead of deleting its published output', async () => {
    // Both First and Second now generate "First", so neither is generated: their published entries would
    // disappear without a retirement record. The guard refuses before writing anything and reports
    // these `removed` violations.
    await expect(runSync(app, { source: 'snapshot', capturedPageIds: ['1:1'] })).rejects.toThrow(
      /removed: .*entry component:First was removed without a retirement record/s
    )
    expect(await readFile(path.join(root, GEN, 'page/First.tsx'), 'utf8')).toBe(generated)
  })
})
