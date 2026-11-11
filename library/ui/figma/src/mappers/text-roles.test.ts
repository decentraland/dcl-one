import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { fixturesDir, snapshotDir, testConfig } from '../../test/snapshot.js'
import type { ComponentSpec, TextRole, ViewNode } from '../ir/components.js'
import { buildComponentSpecs } from './component-spec.js'
import { RESERVED_IDENTIFIERS } from '../generators/naming.js'
import { applyTextRoles } from './text-roles.js'

type Target = { role: string; view: string; path: string; nodeId: string; namePath: string[] }

async function demoNavInput(
  edit?: (data: { components: { name: string; base: { tree: unknown } }[] }) => void
) {
  const config = await testConfig()
  const data = JSON.parse(
    await readFile(path.join(snapshotDir, 'components/Demo Nav.json'), 'utf8')
  )
  edit?.(data)
  return {
    pages: [{ name: 'Demo Nav', category: 'Navigation', data }],
    config,
    collections: [],
    textStyles: [],
    icons: new Map()
  }
}

async function demoNavSpec(): Promise<ComponentSpec> {
  return buildComponentSpecs(await demoNavInput()).find((spec) => spec.name === 'DemoNav')!
}

async function targets(): Promise<Target[]> {
  return JSON.parse(await readFile(path.join(fixturesDir, 'demo-nav-text-roles.json'), 'utf8'))
    .targets
}

function nodeAt(spec: ComponentSpec, view: string, at: string): ViewNode {
  let node = spec.views.find((item) => item.key === view)!.root
  for (const index of at ? at.split('.').map(Number) : []) node = node.children![index]!
  return node
}

// Writes the fixture ids onto the captured views, as a capture that records descendant ids would.
function withIds(spec: ComponentSpec, list: Target[]): ComponentSpec {
  for (const target of list) nodeAt(spec, target.view, target.path).figmaId = target.nodeId
  return spec
}

function rolesFor(list: Target[]): Record<string, TextRole> {
  const roles: Record<string, TextRole> = {}
  for (const { role, nodeId, namePath } of list) {
    roles[role] ??= { type: role === 'notificationCount' ? 'count' : 'string', nodes: [] }
    roles[role].nodes.push({ nodeId, namePath })
  }
  roles.notificationCount!.label = 'notificationsLabel'
  roles.notificationsLabel = { type: 'string', nodes: [] }
  return roles
}

describe('applyTextRoles', () => {
  let spec: ComponentSpec
  let list: Target[]

  beforeEach(async () => {
    list = await targets()
    spec = withIds(await demoNavSpec(), list)
  })

  describe('when every configured node id is captured with its layer names', () => {
    it('should bind all 78 text nodes of the 9 visible roles and declare the label role', () => {
      const roles = applyTextRoles(spec, rolesFor(list))

      expect(list).toHaveLength(78)
      for (const target of list)
        expect(nodeAt(spec, target.view, target.path).slot).toBe(target.role)
      expect(roles.map((role) => role.name).sort()).toEqual([
        'createLabel',
        'creditsBalance',
        'discoverLabel',
        'learnLabel',
        'manaBalance',
        'notificationCount',
        'notificationsLabel',
        'polygonManaBalance',
        'shopLabel',
        'signInLabel'
      ])
      expect(roles.find((role) => role.name === 'notificationCount')).toEqual({
        name: 'notificationCount',
        type: 'count',
        label: 'notificationsLabel'
      })
    })

    it('should tell apart the four tabs that share one layer-name path by their ids', () => {
      const view = 'viewport:largeDesktop|balances:none|theme:dark|signedIn'
      const tabs = list.filter((target) => target.view === view && target.path.startsWith('0.1.'))
      expect(new Set(tabs.map((target) => JSON.stringify(target.namePath))).size).toBe(1)

      applyTextRoles(spec, rolesFor(list))

      expect(tabs.map((target) => nodeAt(spec, view, target.path).slot)).toEqual([
        'discoverLabel',
        'shopLabel',
        'createLabel',
        'learnLabel'
      ])
    })
  })

  describe('when a configured node id is not in the capture', () => {
    it('should fail generation, naming the role and the id', () => {
      const roles = rolesFor(list)
      roles.signInLabel!.nodes[0]!.nodeId = '90001:999'
      expect(() => applyTextRoles(spec, roles)).toThrow(
        'DemoNav text roles: signInLabel: unknown Figma node id 90001:999'
      )
    })
  })

  describe('when the captured layer names differ from the configured path', () => {
    it('should fail generation instead of binding the text', () => {
      const roles = rolesFor(list)
      roles.shopLabel!.nodes[0]!.namePath = [
        ...roles.shopLabel!.nodes[0]!.namePath.slice(0, -1),
        'Renamed'
      ]
      expect(() => applyTextRoles(spec, roles)).toThrow(
        `shopLabel: name path mismatch for ${roles.shopLabel!.nodes[0]!.nodeId}`
      )
    })
  })

  describe('when a node id names a layer that is not text', () => {
    it('should fail generation', () => {
      const view = 'viewport:largeDesktop|balances:none|theme:dark'
      nodeAt(spec, view, '1.0').figmaId = '90001:500'
      const roles = rolesFor(list)
      roles.signInLabel!.nodes.push({
        nodeId: '90001:500',
        namePath: [nodeAt(spec, view, '').name, 'Right', 'SignIn']
      })
      expect(() => applyTextRoles(spec, roles)).toThrow(
        'signInLabel: node 90001:500 is not a text layer'
      )
    })
  })

  describe('when two roles claim the same node', () => {
    it('should fail generation', () => {
      const roles = rolesFor(list)
      roles.shopLabel!.nodes.push(roles.discoverLabel!.nodes[0]!)
      expect(() => applyTextRoles(spec, roles)).toThrow('is already bound to discoverLabel')
    })
  })

  describe('when the same id appears twice in the capture', () => {
    it('should fail generation', () => {
      nodeAt(spec, list[1]!.view, list[1]!.path).figmaId = list[0]!.nodeId
      expect(() => applyTextRoles(spec, rolesFor(list))).toThrow(
        `Figma node id ${list[0]!.nodeId} appears more than once in the capture`
      )
    })
  })

  describe('when a role configuration is inconsistent', () => {
    it.each([
      [
        'a count without a label',
        (roles: Record<string, TextRole>) => delete roles.notificationCount!.label,
        'count role notificationCount needs a label role'
      ],
      [
        'a count labelled by an unknown role',
        (roles: Record<string, TextRole>) => (roles.notificationCount!.label = 'missingLabel'),
        'count role notificationCount names missingLabel, which is not a string role'
      ],
      [
        'a label on a string role',
        (roles: Record<string, TextRole>) => (roles.shopLabel!.label = 'notificationsLabel'),
        'string role shopLabel cannot have a label'
      ],
      [
        'a string role that binds no text',
        (roles: Record<string, TextRole>) => (roles.shopLabel!.nodes = []),
        'shopLabel binds no captured text node'
      ],
      [
        'a label role that binds text',
        (roles: Record<string, TextRole>) =>
          roles.notificationsLabel!.nodes.push(roles.signInLabel!.nodes.pop()!),
        'label role notificationsLabel must not bind captured nodes'
      ],
      [
        'a role named like a variant prop',
        (roles: Record<string, TextRole>) => (roles.theme = roles.shopLabel!),
        'theme collides with an existing prop, slot or generated binding'
      ],
      [
        'two roles that declare the same href',
        (roles: Record<string, TextRole>) => {
          roles.discoverLabel!.href = 'menuHref'
          roles.shopLabel!.href = 'menuHref'
        },
        'discoverLabel and shopLabel declare the same href menuHref'
      ],
      [
        'an href that is not a prop name',
        (roles: Record<string, TextRole>) => (roles.discoverLabel!.href = 'discover-href'),
        'discoverLabel: href discover-href is not a valid prop name'
      ],
      [
        'an href named like a variant prop',
        (roles: Record<string, TextRole>) => (roles.discoverLabel!.href = 'theme'),
        'discoverLabel: href theme collides with an existing prop, slot, role or generated binding'
      ],
      [
        'a role that is not a prop name',
        (roles: Record<string, TextRole>) => (roles['shop-label'] = roles.shopLabel!),
        'shop-label is not a valid prop name'
      ]
    ])('should fail generation for %s', (_case, change, message) => {
      const roles = rolesFor(list)
      change(roles)
      expect(() => applyTextRoles(spec, roles)).toThrow(message)
    })

    // Role names and hrefs become bindings in the generated parameter destructure.
    // Every word the shared validator reserves (keywords, strict-mode and module bindings such as eval, arguments, await).
    it.each([...RESERVED_IDENTIFIERS].map((name) => [name]))(
      'should reject the role name %s',
      (name) => {
        const roles = rolesFor(list)
        roles[name] = roles.shopLabel!
        delete roles.shopLabel
        expect(() => applyTextRoles(spec, roles)).toThrow(
          `DemoNav text roles: ${name} is not a valid prop name`
        )
      }
    )

    it.each([...RESERVED_IDENTIFIERS].map((href) => [href]))(
      'should reject the href %s',
      (href) => {
        const roles = rolesFor(list)
        roles.discoverLabel!.href = href
        expect(() => applyTextRoles(spec, roles)).toThrow(
          `discoverLabel: href ${href} is not a valid prop name`
        )
      }
    )

    it.each([['rest'], ['styles'], ['demoNavViews'], ['demoNavCases'], ['demoNavSuppressedProps']])(
      'should reject the role name %s, which the generated module binds itself',
      (name) => {
        const roles = rolesFor(list)
        roles[name] = roles.shopLabel!
        delete roles.shopLabel
        expect(() => applyTextRoles(spec, roles)).toThrow(
          `${name} collides with an existing prop, slot or generated binding`
        )
      }
    )

    it('should leave the captured views untouched when a later role fails to bind', () => {
      const before = JSON.stringify(spec.views)
      const roles = rolesFor(list)
      roles.signInLabel!.nodes.push({ nodeId: '90001:999', namePath: ['x'] })
      expect(() => applyTextRoles(spec, roles)).toThrow(
        'signInLabel: unknown Figma node id 90001:999'
      )
      expect(JSON.stringify(spec.views)).toBe(before)
    })

    it('should leave the captured views untouched when it fails', () => {
      const before = JSON.stringify(spec.views)
      const roles = rolesFor(list)
      roles.class = roles.shopLabel!
      delete roles.shopLabel
      expect(() => applyTextRoles(spec, roles)).toThrow('class is not a valid prop name')
      expect(JSON.stringify(spec.views)).toBe(before)
    })
  })
})

type RawTree = { id?: string; name?: string; children?: RawTree[] }
type RawDemoNav = {
  name: string
  base: { id: string; name: string; tree: RawTree }
  others: {
    id: string
    name: string
    patches: { at: number[]; set?: Record<string, unknown>; replace?: RawTree }[]
  }[]
}

function rawDemoNav(data: { components: unknown[] }): RawDemoNav {
  return (data.components as RawDemoNav[]).find((component) => component.name === 'DemoNav')!
}

function rawAt(tree: RawTree, at: string): RawTree {
  let node = tree
  for (const index of at ? at.split('.').map(Number) : []) node = node.children![index]!
  return node
}

// A REST capture with ids: every variant is a whole-tree replace carrying its own ids, the root id equals the
// variant envelope id, and only the 78 fixture text layers (plus the roots) carry one.
async function restSpecs(list: Target[], roles?: Record<string, TextRole>) {
  const keyed = await demoNavSpec()
  const variantOf = new Map(keyed.views.map((view, index) => [view.key, index]))
  const input = await demoNavInput((data) => {
    const demoNav = rawDemoNav(data)
    const trees = [
      demoNav.base.tree,
      ...demoNav.others.map((variant) => variant.patches[0]!.replace!)
    ]
    demoNav.base.tree.id = demoNav.base.id
    demoNav.others.forEach((variant, index) => (trees[index + 1]!.id = variant.id))
    for (const target of list)
      rawAt(trees[variantOf.get(target.view)!]!, target.path).id = target.nodeId
  })
  if (roles)
    input.config.components.DemoNav = { ...input.config.components.DemoNav, textRoles: roles }
  return buildComponentSpecs(input).find((item) => item.name === 'DemoNav')!
}

describe('buildComponentSpecs with text roles', () => {
  let list: Target[]

  beforeEach(async () => {
    list = await targets()
  })

  describe('when a REST capture carries the 78 text ids and the configuration binds the roles', () => {
    it('should bind every role and keep the ids out of the spec', async () => {
      const spec = await restSpecs(list, rolesFor(list))

      for (const target of list)
        expect(nodeAt(spec, target.view, target.path).slot).toBe(target.role)
      expect(spec.textRoles).toHaveLength(10)
      expect(JSON.stringify(spec)).not.toContain('90001:')
      const withId: string[] = []
      const visit = (node: ViewNode) => {
        if (node.figmaId) withId.push(node.figmaId)
        node.children?.forEach(visit)
      }
      spec.views.forEach((view) => visit(view.root))
      expect(withId).toEqual([])
    })
  })

  describe('when a capture carries ids but nothing binds them', () => {
    it('should produce the same spec as a capture without ids', async () => {
      expect(JSON.stringify(await restSpecs(list))).toBe(JSON.stringify(await demoNavSpec()))
    })
  })

  describe('when a legacy partial variant inherits the base tree', () => {
    // The mobile signed-out variant becomes a partial patch of the base, whose text has the id 90001:1.
    async function partialSpec(
      patches: RawDemoNav['others'][number]['patches'],
      roles: Record<string, TextRole>
    ) {
      const input = await demoNavInput((data) => {
        const demoNav = rawDemoNav(data)
        demoNav.base.tree.id = demoNav.base.id
        rawAt(demoNav.base.tree, '1.0.0').id = '90001:1'
        const variant = demoNav.others.find((item) =>
          item.name.startsWith('viewport=mobile, signed-in=false, balances=none, theme=dark')
        )!
        variant.patches = [{ at: [], set: { name: variant.name } }, ...patches]
      })
      input.config.components.DemoNav = { ...input.config.components.DemoNav, textRoles: roles }
      return buildComponentSpecs(input).find((item) => item.name === 'DemoNav')!
    }
    const base = 'viewport:largeDesktop|balances:none|theme:dark'
    const mobile = 'viewport:mobile|balances:none|theme:dark'
    const namePath = (root: string) => [root, 'Right', 'SignIn', 'Label']

    it('should not give the variant the base node id, and leave its unmapped text unbound without failing', async () => {
      const spec = await partialSpec([], {
        signInLabel: {
          type: 'string',
          nodes: [
            {
              nodeId: '90001:1',
              namePath: namePath(
                'viewport=large-desktop, signed-in=false, balances=none, theme=dark'
              )
            }
          ]
        }
      })
      expect(nodeAt(spec, base, '1.0.0').slot).toBe('signInLabel')
      expect(nodeAt(spec, mobile, '1.0.0').slot).not.toBe('signInLabel')
    })

    it('should bind the variant node through its own set.id', async () => {
      const spec = await partialSpec([{ at: [1, 0, 0], set: { id: '90001:2' } }], {
        signInLabel: {
          type: 'string',
          nodes: [
            {
              nodeId: '90001:1',
              namePath: namePath(
                'viewport=large-desktop, signed-in=false, balances=none, theme=dark'
              )
            },
            {
              nodeId: '90001:2',
              namePath: namePath('viewport=mobile, signed-in=false, balances=none, theme=dark')
            }
          ]
        }
      })
      expect(nodeAt(spec, base, '1.0.0').slot).toBe('signInLabel')
      expect(nodeAt(spec, mobile, '1.0.0').slot).toBe('signInLabel')
    })

    it('should fail generation when a role points at a variant node whose only id was inherited', async () => {
      await expect(
        partialSpec([], {
          signInLabel: {
            type: 'string',
            nodes: [
              {
                nodeId: '90001:1',
                namePath: namePath(
                  'viewport=large-desktop, signed-in=false, balances=none, theme=dark'
                )
              },
              {
                nodeId: '90001:1',
                namePath: namePath('viewport=mobile, signed-in=false, balances=none, theme=dark')
              }
            ]
          }
        })
      ).rejects.toThrow('signInLabel: name path mismatch for 90001:1')
    })
  })

  describe('when figma.config.json binds an id the capture lacks', () => {
    it('should fail generation', async () => {
      const input = await demoNavInput()
      input.config.components.DemoNav = {
        ...input.config.components.DemoNav,
        textRoles: {
          signInLabel: { type: 'string', nodes: [{ nodeId: '90001:1', namePath: ['x'] }] }
        }
      }
      expect(() => buildComponentSpecs(input)).toThrow(
        'DemoNav text roles: signInLabel: unknown Figma node id 90001:1'
      )
    })
  })
})

describe('configured generated names', () => {
  // Every generated name a configuration states follows the same identifier rule as text roles.
  it.each([
    ['viewport', 'class'],
    ['signed-in', 'eval']
  ])('should fail generation when propNames maps %s to %s', async (figmaName, name) => {
    const input = await demoNavInput()
    input.config.components.DemoNav = {
      ...input.config.components.DemoNav,
      propNames: { [figmaName]: name }
    }
    expect(() => buildComponentSpecs(input)).toThrow(
      `Invalid generated TypeScript identifier: ${JSON.stringify(name)}`
    )
  })

  it('should keep a valid configured prop name as written', async () => {
    const input = await demoNavInput()
    input.config.components.DemoNav = {
      ...input.config.components.DemoNav,
      propNames: { viewport: 'screen' }
    }
    const spec = buildComponentSpecs(input).find((item) => item.name === 'DemoNav')!
    expect(spec.props.map((prop) => prop.name)).toContain('screen')
  })
})
