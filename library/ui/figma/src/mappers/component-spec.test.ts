import { describe, expect, it } from 'vitest'
import { componentSpec, componentSpecs, projectRoot, snapshotSource } from '../../test/snapshot.js'
import { loadConfig } from '../config.js'
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
        figmaName: 'Button-Contained-Text',
        page: 'Buttons',
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
        background: 'var(--dcl-primary-primary)',
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
        { path: [], decls: [['background', 'var(--dcl-primary-primary-light-1)']] }
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

      expect(icon.icon).toMatchObject({ component: 'GeneralStar', size: 20 })
      expect(icon.slot).toBe('icon')
    })

    it('should follow a configured legacy icon alias and leave a note', async () => {
      const spec = await componentSpec('SortByButton')

      expect(spec.notes).toContain('legacy-icon:Icn/DownArrow->general/down-arrow')
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

  describe('when rendering the captured web input caret', () => {
    it('should preserve the visible caret dimensions', async () => {
      const spec = await componentSpec('InputWebCreatorTools')
      const view = spec.views.find((entry) => find(entry.root, (node) => node.name === 'Cursor'))!
      const caret = find(view.root, (node) => node.name === 'Cursor')!

      expect([declarations(caret).width, declarations(caret).height]).toEqual(['2px', '20px'])
    })
  })

  describe('when a layer cannot be reproduced', () => {
    it('should flag an instance of a component that was not captured as needs-manual', async () => {
      const friend = await componentSpec('FriendCTA')

      expect(friend.needsManual).toContain('unresolved-instance:ContextualMenuIcons')
    })

    it('should prevent promotion of a component whose hidden variants were excluded', async () => {
      const source = snapshotSource()
      const buttons = (await source.getComponentPage('Buttons'))!
      const raw = structuredClone(buttons.components[0]!)
      raw.hiddenVariants = [{ id: 'hidden-variant', name: 'State=Hidden' }]
      const [spec] = buildComponentSpecs({
        pages: [{ name: 'Buttons', category: 'Inputs', data: { ...buttons, components: [raw] } }],
        config: await loadConfig(projectRoot),
        collections: await source.getTokenCollections(),
        textStyles: await source.getTextStyles(),
        icons: new Map()
      })
      expect(spec!.needsManual).toContain('hidden-variants-excluded: hidden-variant')
    })

    it('should flag a vector captured without its artwork as a custom glyph', async () => {
      const source = snapshotSource()
      const buttons = (await source.getComponentPage('Buttons'))!
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
          { name: 'Buttons', category: 'Inputs', data: { ...buttons, components: [notify] } }
        ],
        config: await loadConfig(projectRoot),
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
          config: await loadConfig(projectRoot),
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
        (await componentSpec('PageIndicator')).needsManual.some((r) =>
          r.startsWith('slot-property')
        )
      ).toBe(true)
    })
  })

  describe('when building every captured page', () => {
    it('should produce one spec per Figma set with unique names', async () => {
      const specs = await componentSpecs()

      // The frozen capture keeps nine same-named pairs of distinct sets: they fail closed and are excluded.
      expect(specs.length).toBeGreaterThanOrEqual(176)
      expect(new Set(specs.map((spec) => spec.name)).size).toBe(specs.length)
    })
  })
})
