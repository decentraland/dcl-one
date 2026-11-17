import { describe, expect, it } from 'vitest'
import { componentSpec, componentSpecs, snapshotSource, testConfig } from '../../test/snapshot.js'
import type { ViewNode } from '../ir/components.js'
import type { RawNode } from '../ir/raw-components.js'
import { buildComponentSpecs } from './component-spec.js'

const find = (node: ViewNode, predicate: (node: ViewNode) => boolean): ViewNode | undefined => {
  if (predicate(node)) return node
  for (const child of node.children ?? []) {
    const found = find(child, predicate)
    if (found) return found
  }
  return undefined
}

const declarations = (node: ViewNode) => Object.fromEntries(node.css.decls)

describe('buildComponentSpecs', () => {
  describe('when building the main Button', () => {
    it('should name it Button, expose variant and disabled and default to primary', async () => {
      const spec = await componentSpec('Button')

      expect(spec).toMatchObject({
        figmaName: 'Synthetic Button',
        page: 'Demo Controls',
        category: 'Inputs',
        tag: 'button',
        defaultKey: 'variant:primary',
        needsManual: []
      })
      expect(spec.props.map((prop) => prop.name)).toEqual(['variant', 'disabled'])
      expect(spec.views.map((view) => view.key)).toEqual([
        'variant:primary',
        'variant:primary|disabled',
        'variant:secondary',
        'variant:tertiary',
        'variant:quaternary'
      ])
    })

    it('should resolve colours, spacing and radius to design tokens', async () => {
      const { views } = await componentSpec('Button')
      const root = views[0]!.root

      expect(declarations(root)).toMatchObject({
        background: 'var(--dcl-brand-primary)',
        'border-radius': 'var(--dcl-radius-md)',
        padding: 'var(--dcl-space-12) var(--dcl-space-24) var(--dcl-space-12) var(--dcl-space-24)'
      })
    })

    it('should map the label to a text slot with its text style and measured line height', async () => {
      const spec = await componentSpec('Button')
      const label = find(spec.views[0]!.root, (node) => node.kind === 'text')!

      expect(label).toMatchObject({ slot: 'label', text: 'Label' })
      expect(label.css.textStyle).toBe('label/md-caps')
      expect(declarations(label)['line-height']).toBe('22px')
      expect(spec.slots).toEqual([{ name: 'label', kind: 'text', default: 'Label' }])
    })

    it('should turn the hover variant into a rule that only lists what changes', async () => {
      const { views } = await componentSpec('Button')

      expect(views[0]!.hover).toEqual([
        { path: [], decls: [['background', 'var(--dcl-brand-primary-light)']] }
      ])
    })
  })

  describe('when a component embeds another captured component', () => {
    it('should reference it by component id with the props of the instantiated variant', async () => {
      const spec = await componentSpec('SwitchTitle')
      const ref = find(
        spec.views[0]!.root,
        (node) => node.kind === 'ref' && node.ref?.component === 'Switch'
      )

      expect(ref?.ref?.props).toMatchObject({ checked: true })
    })
  })

  describe('when a layer is an icon of the generated icon library', () => {
    it('should render the library icon with its colour and size', async () => {
      const spec = await componentSpec('InterestedCTA')
      const icon = find(spec.views[0]!.root, (node) => node.kind === 'icon')!

      expect(icon.icon).toMatchObject({ component: 'SampleStar', size: 20 })
      expect(icon.slot).toBe('icon')
    })

    it('should follow a configured legacy icon alias and leave a note', async () => {
      const spec = await componentSpec('SortByButton')

      expect(spec.notes).toContain('legacy-icon:LegacyArrow->sample/arrow-down')
      expect(spec.needsManual).toEqual([])
    })
  })

  describe('when a layer is captured artwork', () => {
    it('should paint a captured svg as an absolutely positioned background at its Figma offset', async () => {
      const spec = await componentSpec('Crosshair')
      const dot = find(spec.views[0]!.root, (node) => node.path.join('.') === '1')!
      const css = declarations(dot)

      expect(spec.needsManual).toEqual([])
      expect(css.position).toBe('absolute')
      expect([css.left, css.top]).toEqual(['12px', '12px'])
      expect(css.background).toMatch(/^url\("data:image\/svg\+xml,/)
    })

    it('should keep artwork that was too heavy to embed as a needs-manual reason', async () => {
      const spec = await componentSpec('Cursor3rdPerson')

      expect(spec.needsManual.some((reason) => reason.startsWith('heavy-artwork:'))).toBe(true)
    })
  })

  describe('when a text layer has a fixed width and spans several lines', () => {
    it('should keep its width and let it wrap instead of forcing a single line', async () => {
      const spec = await componentSpec('Toast')
      const view = spec.views.find((entry) => entry.key === 'variant:errorClose')!
      const text = find(view.root, (node) => node.kind === 'text')!
      const css = declarations(text)

      expect(css.width).toBe('340px')
      expect(css['white-space']).toBeUndefined()
      expect(css['line-height']).toBeUndefined()
    })

    it('should keep single-line labels on one line', async () => {
      const spec = await componentSpec('Toast')
      const text = find(spec.views[0]!.root, (node) => node.kind === 'text')!

      expect(declarations(text)['white-space']).toBe('nowrap')
    })
  })

  describe('when rendering a captured text input caret', () => {
    it('should preserve the visible caret dimensions', async () => {
      const spec = await componentSpec('CaretInput')
      const view = spec.views.find((entry) => find(entry.root, (node) => node.name === 'Cursor'))!
      const caret = find(view.root, (node) => node.name === 'Cursor')!

      expect([declarations(caret).width, declarations(caret).height]).toEqual(['2px', '20px'])
    })
  })

  describe('when a layer cannot be reproduced', () => {
    it('should flag an instance of a component that was not captured as needs-manual', async () => {
      const friend = await componentSpec('FriendCTA')

      expect(friend.needsManual).toContain('unresolved-instance:MenuGlyphs')
    })

    it('should resolve a per-variant icon alias of the same glyph set to the library icon', async () => {
      const friend = await componentSpec('FriendCTA')
      const reject = friend.views.find((view) => view.key === 'variant:reject')!

      expect(friend.notes).toContain('legacy-icon:MenuGlyphs->sample/heart')
      expect(find(reject.root, (node) => node.kind === 'icon')?.icon?.component).toBe('SampleHeart')
    })

    it('should prevent promotion of a component whose hidden variants were excluded', async () => {
      const source = snapshotSource()
      const buttons = (await source.getComponentPage('Demo Controls'))!
      const raw = structuredClone(buttons.components[0]!)
      raw.hiddenVariants = [{ id: 'hidden-variant', name: 'State=Hidden' }]
      const [spec] = buildComponentSpecs({
        pages: [
          { name: 'Demo Controls', category: 'Inputs', data: { ...buttons, components: [raw] } }
        ],
        config: await testConfig(),
        collections: await source.getTokenCollections(),
        textStyles: await source.getTextStyles(),
        icons: new Map()
      })
      expect(spec!.needsManual).toContain('hidden-variants-excluded: hidden-variant')
    })

    it('should flag a vector captured without its artwork as a custom glyph', async () => {
      const source = snapshotSource()
      const buttons = (await source.getComponentPage('Demo Controls'))!
      const notify = structuredClone(
        buttons.components.find((component) => component.name === 'Notify')!
      )
      const strip = (node: RawNode): boolean => {
        if (node.type === 'VECTOR' && node.svg !== undefined) {
          delete node.svg
          delete node.sum
          return true
        }
        return (node.children ?? []).some(strip)
      }
      expect(strip(notify.base.tree)).toBe(true)

      const [spec] = buildComponentSpecs({
        pages: [
          { name: 'Demo Controls', category: 'Inputs', data: { ...buttons, components: [notify] } }
        ],
        config: await testConfig(),
        collections: await source.getTokenCollections(),
        textStyles: await source.getTextStyles(),
        icons: new Map()
      })

      expect(spec!.needsManual.some((reason) => reason.startsWith('custom-glyph:'))).toBe(true)
    })

    it('should hide a layer that a state adds in the rest view and reveal it on hover', async () => {
      const spec = await componentSpec('SquaredButton')
      const view = spec.views[0]!
      const hidden = find(view.root, (node) => node.path[node.path.length - 1]! >= 1000)!

      expect(spec.needsManual.some((reason) => reason.startsWith('state-adds-layers'))).toBe(false)
      expect(declarations(hidden).display).toBe('none')
      expect(view.hover?.some((rule) => rule.path.join('.') === hidden.path.join('.'))).toBe(true)
    })

    it('should carry the hover motion Figma defines on the rest variant', async () => {
      const source = snapshotSource()
      const { page } = await componentSpec('SquaredButton')
      const data = (await source.getComponentPage(page))!
      const raw = structuredClone(data.components.find((c) => c.name === 'SquaredButton')!)
      const specOf = async () =>
        buildComponentSpecs({
          pages: [{ name: page, category: 'Inputs', data: { ...data, components: [raw] } }],
          config: await testConfig(),
          collections: await source.getTokenCollections(),
          textStyles: await source.getTextStyles(),
          icons: new Map()
        })[0]!
      const before = await specOf()
      const view = before.views[0]!
      const hoverId = [raw.base, ...raw.others].find(
        (variant) =>
          variant.id !== view.figmaId &&
          variant.name === view.figmaName.replace('Status=Default', 'Status=Hover')
      )?.id
      expect(view.motion).toBeUndefined()
      expect(hoverId).toBeDefined()

      const rest = [raw.base, ...raw.others].find((variant) => variant.id === view.figmaId)!
      rest.interactions = [
        {
          trigger: 'ON_HOVER',
          destinationId: hoverId!,
          transition: { type: 'SMART_ANIMATE', duration: 0.25, easing: 'EASE_IN_AND_OUT' }
        }
      ]
      expect((await specOf()).views[0]!.motion).toEqual({
        hover: { duration: 250, easing: 'ease-in-out' }
      })

      rest.interactions[0]!.transition!.easing = 'BOUNCY'
      const flagged = await specOf()
      expect(flagged.views[0]!.motion).toBeUndefined()
      expect(flagged.needsManual).toEqual(before.needsManual)
      expect(flagged.notes).toContain(
        `motion-not-generated:unsupported-easing:BOUNCY:${view.key}:hover`
      )
    })

    it('should flag components whose capture was truncated or that use Figma slots', async () => {
      expect(
        (await componentSpec('Slot')).needsManual.some((r) => r.startsWith('truncated-capture'))
      ).toBe(true)
      expect(
        (await componentSpec('PageDots')).needsManual.some((r) => r.startsWith('slot-property'))
      ).toBe(true)
    })
  })

  describe('when a layout overlaps its children with negative spacing', () => {
    it('should pull every later child back by a tokenized negative margin and keep the layer order', async () => {
      const spec = await componentSpec('AvatarStack')
      const [first, second, third] = spec.views[0]!.root.children!

      expect(declarations(spec.views[0]!.root).gap).toBeUndefined()
      expect(declarations(first!)['margin-left']).toBeUndefined()
      expect(declarations(second!)['margin-left']).toBe('calc(var(--dcl-space-8) * -1)')
      expect([first, second, third].map((node) => declarations(node!)['z-index'])).toEqual([
        '1',
        '2',
        '3'
      ])
    })
  })

  describe('when a layout distributes its children with space-between', () => {
    it('should ignore the captured item spacing instead of turning it into a minimum gap', async () => {
      const spec = await componentSpec('SwitchTitle')
      const css = declarations(spec.views[0]!.root)

      expect(css['justify-content']).toBe('space-between')
      expect(css.gap).toBeUndefined()
    })
  })

  describe('when building every captured page', () => {
    it('should produce one spec per Figma set with unique names', async () => {
      const specs = await componentSpecs()

      expect(specs.length).toBe(27)
      expect(new Set(specs.map((spec) => spec.name)).size).toBe(specs.length)
    })

    it('should leave out both distinct sets that generate the same name', async () => {
      const source = snapshotSource()
      const badges = await Promise.all(
        ['Demo Cursors', 'Demo Feedback'].map(async (page) =>
          (await source.getComponentPage(page))!.components.filter(
            (component) => component.name === 'Badge'
          )
        )
      )

      expect(badges.map((list) => list.map((component) => component.id))).toEqual([
        ['22:30'],
        ['40:60']
      ])
      expect((await componentSpecs()).some((spec) => spec.name === 'Badge')).toBe(false)
    })
  })
})
