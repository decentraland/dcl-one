import { readFile, rm } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
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
  const pages = [
    page('Buttons', [
      component('16326:4282', 'IconButton'),
      component('15785:43182', 'Icon Button'),
      component('10:1', 'Close Button')
    ])
  ]

  it('should generate neither and report one sorted diagnostic', () => {
    const { specs, collisions, files } = build(pages)

    expect(specs.map((spec) => spec.name)).toEqual(['CloseButton'])
    expect(paths(files).filter((file) => /IconButton/i.test(file))).toEqual([])
    expect(index(files)).not.toMatch(/IconButton/)
    expect(collisions.map(collisionReason)).toEqual([
      'name-collision:IconButton: 15785:43182 "Icon Button", 16326:4282 "IconButton"'
    ])
  })

  it('should produce byte-identical output and diagnostics for the reversed capture order', () => {
    const reversed = [page('Buttons', [...pages[0]!.data.components].reverse() as never)]
    const forward = build(pages)
    const backward = build(reversed)

    expect(backward.files.map((file) => [file.path, file.content]).sort()).toEqual(
      forward.files.map((file) => [file.path, file.content]).sort()
    )
    expect(backward.collisions).toEqual(forward.collisions)
  })

  it('should generate both when distinct aliases resolve the collision', () => {
    const { specs, collisions } = build(pages, {
      'Icon Button': 'IconButtonOutlined',
      IconButton: 'IconButtonFilled'
    })

    expect(specs.map((spec) => [spec.figmaId, spec.name])).toEqual([
      ['16326:4282', 'IconButtonFilled'],
      ['15785:43182', 'IconButtonOutlined'],
      ['10:1', 'CloseButton']
    ])
    expect(collisions).toEqual([])
  })

  it('should treat names that differ only in case as a collision', () => {
    const { specs, collisions } = build([
      page('A', [component('1:1', 'Iconbutton'), component('1:2', 'IconButton')])
    ])

    expect(specs).toEqual([])
    expect(collisions).toHaveLength(1)
  })
})

describe('when names do not collide', () => {
  it('should keep every generated name unchanged', () => {
    const { specs, collisions } = build([
      page('Buttons', [component('1:1', 'Close Button'), component('1:2', 'Button-Plain')])
    ])

    expect(specs.map((spec) => spec.name)).toEqual(['CloseButton', 'ButtonPlain'])
    expect(collisions).toEqual([])
  })
})

describe('when the same Figma id is captured on two pages', () => {
  it('should generate it once, from its first configured page, with no diagnostic', () => {
    const shared = component('5:5', 'NavMenuTab')
    const { specs, collisions, files } = build([
      page('Nav bar', [shared]),
      page('Bars legacy', [shared])
    ])

    expect(specs.map((spec) => [spec.name, spec.page])).toEqual([['NavMenuTab', 'Nav bar']])
    expect(paths(files).filter((file) => file.endsWith('NavMenuTab.tsx'))).toEqual([
      'components/nav-bar/NavMenuTab.tsx'
    ])
    expect(index(files).match(/NavMenuTab'/g)).toHaveLength(1)
    expect(collisions).toEqual([])
  })

  it.each([
    ['different raw names', {}],
    ['different aliases', { 'Nav Menu Tab': 'NavTabPrimary', 'Legacy Tab': 'NavTabLegacy' }]
  ])('should generate it once under %s', (_, aliases) => {
    const { specs, collisions, files } = build(
      [
        page('Nav bar', [component('5:5', 'Nav Menu Tab')]),
        page('Bars legacy', [component('5:5', 'Legacy Tab')])
      ],
      aliases
    )

    expect(specs.map((spec) => [spec.figmaId, spec.page])).toEqual([['5:5', 'Nav bar']])
    const name = specs[0]!.name
    expect(paths(files).filter((file) => file.endsWith('.tsx'))).toEqual([
      `components/nav-bar/${name}.tsx`
    ])
    expect(index(files).match(/^export \* from '\.\/[a-z-]+\//gm)).toEqual([
      "export * from './nav-bar/"
    ])
    expect(collisions).toEqual([])
  })

  it('should still fail closed when another id takes the same name, listing distinct ids', () => {
    const shared = component('9141:1615', 'NavBar')
    const { specs, collisions } = build([
      page('Chat', [shared]),
      page('Bars', [component('15599:30721', 'NavBar')]),
      page('Legacy', [shared])
    ])

    expect(specs).toEqual([])
    expect(collisions.map(collisionReason)).toEqual([
      'name-collision:NavBar: 9141:1615 "NavBar", 15599:30721 "NavBar"'
    ])
  })
})

describe('when a previously generated component starts colliding', () => {
  const temps: string[] = []
  afterEach(async () => {
    for (const dir of temps.splice(0)) await rm(dir, { recursive: true, force: true })
  })

  it('should block the removal instead of deleting its published output', async () => {
    const { root, app } = await lifecycleFixture()
    temps.push(root)
    const snapshot = await json(root, `${SNAP}/components/Page.json`)
    snapshot.components[1].name = 'First'
    snapshot.components[1].base.name = 'First'
    await put(root, `${SNAP}/components/Page.json`, snapshot)
    const generated = await readFile(path.join(root, GEN, 'page/First.tsx'), 'utf8')

    // Both First and Second now generate "First", so neither is generated: their published entries would
    // disappear without a retirement record. The guard refuses before writing anything and reports
    // these `removed` violations.
    await expect(runSync(app, { source: 'snapshot', capturedPageIds: ['1:1'] })).rejects.toThrow(
      /removed: .*entry component:First was removed without a retirement record/s
    )
    expect(await readFile(path.join(root, GEN, 'page/First.tsx'), 'utf8')).toBe(generated)
  })
})
