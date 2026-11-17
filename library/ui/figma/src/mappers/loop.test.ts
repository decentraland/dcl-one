import { beforeEach, describe, expect, it } from 'vitest'
import { snapshotSource, testConfig } from '../../test/snapshot.js'
import { generateComponents } from '../generators/components.js'
import type { ComponentSpec } from '../ir/components.js'
import type { RawComponent, RawInteraction } from '../ir/raw-components.js'
import { buildComponentSpecs } from './component-spec.js'

// The Figma Skeleton: a 200x112 clip whose Sheen band moves from x=-200 (phase=start) to x=200
// (phase=end) with Smart Animate, then jumps back with no transition.
function skeleton(): RawComponent {
  return {
    kind: 'set',
    id: 'set',
    name: 'Skeleton',
    section: [],
    x: 0,
    y: 0,
    properties: { phase: { type: 'VARIANT', default: 'start', options: ['start', 'end'] } },
    base: {
      id: 'start',
      name: 'phase=start',
      props: { phase: 'start' },
      interactions: [forward()],
      tree: {
        type: 'COMPONENT',
        name: 'phase=start',
        w: 200,
        h: 112,
        clipsContent: true,
        fills: [{ type: 'SOLID', hex: '#2b2730' }],
        children: [
          {
            type: 'RECTANGLE',
            name: 'Sheen',
            w: 200,
            h: 112,
            x: -200,
            y: 0,
            absolute: true,
            fills: [{ type: 'SOLID', hex: '#ffffff', opacity: 0.2 }]
          }
        ]
      }
    },
    others: [
      {
        id: 'end',
        name: 'phase=end',
        props: { phase: 'end' },
        interactions: [back()],
        // The root is renamed like every Figma variant; only the Sheen moves.
        patches: [
          { at: [], set: { name: 'phase=end' } },
          { at: [0], set: { x: 200 } }
        ]
      }
    ]
  }
}

function forward(): RawInteraction {
  return {
    trigger: 'AFTER_TIMEOUT',
    timeout: 0.001,
    destinationId: 'end',
    transition: { type: 'SMART_ANIMATE', duration: 1.5, easing: 'LINEAR' }
  }
}

function back(): RawInteraction {
  return { trigger: 'AFTER_TIMEOUT', timeout: 0.001, destinationId: 'start' }
}

describe('timed variant loops', () => {
  let raw: RawComponent
  let specOf: () => Promise<ComponentSpec>

  beforeEach(() => {
    raw = skeleton()
    specOf = async () => {
      const source = snapshotSource()
      return buildComponentSpecs({
        pages: [
          {
            name: 'Spinner & Skeleton Animation',
            category: 'Components',
            data: { page: { id: 'page', name: 'Spinner & Skeleton Animation' }, components: [raw] }
          }
        ],
        config: await testConfig(),
        collections: await source.getTokenCollections(),
        textStyles: await source.getTextStyles(),
        icons: new Map([['Glyph', 'IconGlyph']])
      })[0]!
    }
  })

  const notesOf = (spec: ComponentSpec) =>
    spec.notes.filter((note) => note.startsWith('motion-not-generated:unsupported-loop'))

  describe('when the set is the supported two-variant loop', () => {
    it('should put the loop on the rest variant and leave the other variant static', async () => {
      const spec = await specOf()
      const [start, end] = spec.views

      expect(start!.key).toBe('phase:start')
      expect(start!.loop).toEqual({
        to: 'phase:end',
        duration: 1500,
        easing: 'linear',
        tracks: [{ path: [0], from: [['left', '-200px']], to: [['left', '200px']] }]
      })
      expect(end!.loop).toBeUndefined()
      expect(notesOf(spec)).toEqual([])
      expect(spec.needsManual).toEqual([])
    })

    it('should treat a raw wait of 0 as immediate', async () => {
      raw.base.interactions![0]!.timeout = 0
      raw.others[0]!.interactions![0]!.timeout = 0

      expect((await specOf()).views[0]!.loop?.duration).toBe(1500)
    })

    it('should write a custom curve as cubic-bezier', async () => {
      raw.base.interactions![0]!.transition = {
        type: 'SMART_ANIMATE',
        duration: 1,
        easing: 'CUSTOM_CUBIC_BEZIER',
        bezier: [0.4, 0, 0.2, 1]
      }

      expect((await specOf()).views[0]!.loop?.easing).toBe('cubic-bezier(0.4, 0, 0.2, 1)')
    })
  })

  describe('when the timed change is not the supported shape', () => {
    const unsupported = async (reason: string) => {
      const spec = await specOf()
      expect(spec.views.every((view) => view.loop === undefined)).toBe(true)
      expect(notesOf(spec)).toEqual([`motion-not-generated:unsupported-loop:${reason}:phase:start`])
    }

    it('should report a one-way timed change', async () => {
      raw.others[0]!.interactions = undefined
      await unsupported('no-return')
    })

    it('should report an animated return', async () => {
      raw.others[0]!.interactions![0]!.transition = {
        type: 'DISSOLVE',
        duration: 0.3,
        easing: 'LINEAR'
      }
      await unsupported('reset-not-instant:DISSOLVE')
    })

    it('should report an instant or non Smart Animate forward change', async () => {
      raw.base.interactions![0]!.transition = undefined
      await unsupported('unsupported-transition:INSTANT')
    })

    it('should report an easing CSS cannot reproduce', async () => {
      raw.base.interactions![0]!.transition!.easing = 'GENTLE_SPRING'
      await unsupported('unsupported-easing:GENTLE_SPRING')
    })

    it('should report a destination outside the set, and the reset left pointing at a chain', async () => {
      raw.base.interactions![0]!.destinationId = 'elsewhere'
      expect(notesOf(await specOf())).toEqual([
        'motion-not-generated:unsupported-loop:destination-outside-set:phase:start',
        'motion-not-generated:unsupported-loop:more-than-two-variants:phase:end'
      ])
    })

    it('should report several timed changes from the same variant', async () => {
      raw.base.interactions!.push({ ...forward() })
      await unsupported('several-timed-changes')
    })

    it('should report a change that is not geometry or opacity', async () => {
      raw.others[0]!.patches = [
        { at: [0], set: { fills: [{ type: 'SOLID', hex: '#ff0000', opacity: 1 }] } }
      ]
      await unsupported('non-geometry-change:background')
    })

    it('should report a layer that appears or disappears', async () => {
      raw.others[0]!.patches = [{ at: [0], set: { hidden: true } }]
      await unsupported('layers-change')
    })

    it('should report two variants that are not one property apart', async () => {
      raw.properties.size = { type: 'VARIANT', default: 'sm', options: ['sm', 'lg'] }
      raw.base.name = 'phase=start, size=sm'
      raw.base.props = { phase: 'start', size: 'sm' }
      raw.others[0]!.name = 'phase=end, size=lg'
      raw.others[0]!.props = { phase: 'end', size: 'lg' }
      const spec = await specOf()
      expect(notesOf(spec)).toEqual([
        `motion-not-generated:unsupported-loop:not-sibling-variants:${spec.views[0]!.key}`
      ])
    })
  })

  describe('when the timing cannot be reproduced exactly', () => {
    const unsupported = async (reason: string) => {
      const spec = await specOf()
      expect(spec.views.every((view) => view.loop === undefined)).toBe(true)
      expect(notesOf(spec)).toEqual([`motion-not-generated:unsupported-loop:${reason}:phase:start`])
    }

    it('should report a wait missing from the capture instead of assuming it is immediate', async () => {
      delete raw.base.interactions![0]!.timeout
      await unsupported('missing-timeout')
    })

    it('should report a missing wait on the change back', async () => {
      delete raw.others[0]!.interactions![0]!.timeout
      await unsupported('missing-timeout')
    })

    it.each([-1, Number.NaN, Number.POSITIVE_INFINITY])(
      'should report an invalid wait of %s',
      async (timeout) => {
        raw.others[0]!.interactions![0]!.timeout = timeout
        await unsupported('invalid-timeout')
      }
    )

    it.each([0.5, 20, 1000])(
      'should report a longer wait of %s whose unit is not established',
      async (timeout) => {
        raw.base.interactions![0]!.timeout = timeout
        await unsupported('unverified-wait')
      }
    )

    it.each([0, -1, 0.0004, Number.NaN])(
      'should report a transition duration of %s seconds',
      async (duration) => {
        raw.base.interactions![0]!.transition!.duration = duration
        await unsupported('invalid-duration')
      }
    )

    it.each([
      [[1.2, 0, 0.2, 1]],
      [[0.4, 0, -0.1, 1]],
      [[0.4, 0, 0.2, Number.NaN]],
      [[0.4, 0, 0.2]]
    ])('should report the cubic-bezier %j that CSS rejects', async (bezier) => {
      raw.base.interactions![0]!.transition = {
        type: 'SMART_ANIMATE',
        duration: 1,
        easing: 'CUSTOM_CUBIC_BEZIER',
        // Malformed captures included on purpose (wrong length, non-finite).
        bezier: bezier as [number, number, number, number]
      }
      await unsupported('unsupported-easing:CUSTOM_CUBIC_BEZIER')
    })
  })

  describe('when something besides geometry changes with the move', () => {
    const unsupported = async (reason: string) => {
      const spec = await specOf()
      expect(spec.views.every((view) => view.loop === undefined)).toBe(true)
      expect(notesOf(spec)).toEqual([`motion-not-generated:unsupported-loop:${reason}:phase:start`])
    }
    const addLayer = (layer: RawComponent['base']['tree']) => raw.base.tree.children!.push(layer)
    const label = (text: string, extra = {}) => ({
      type: 'TEXT',
      name: 'Label',
      w: 60,
      h: 16,
      x: 0,
      y: 0,
      absolute: true,
      text,
      fontSize: 12,
      ...extra
    })

    it('should report a text that changes while it moves', async () => {
      addLayer(label('Before'))
      raw.others[0]!.patches = [{ at: [1], set: { x: 100, text: 'After' } }]
      await unsupported('non-geometry-change:text')
    })

    it('should report text runs that change while they move', async () => {
      const run = (text: string) => ({ text, fontSize: 12 })
      addLayer(label('AB', { textRuns: [run('A'), run('B')] }))
      raw.others[0]!.patches = [
        { at: [1], set: { x: 100, text: 'AC', textRuns: [run('A'), run('C')] } }
      ]
      await unsupported('non-geometry-change:text')
    })

    it('should report a text style that changes while it moves', async () => {
      const styles = (await snapshotSource().getTextStyles()).map((style) => style.name)
      addLayer(label('Same', { textStyle: styles[0] }))
      raw.others[0]!.patches = [{ at: [1], set: { x: 100, textStyle: styles[1] } }]
      await unsupported('non-geometry-change:textStyle')
    })

    it('should report a component reference whose props change while it moves', async () => {
      addLayer({ type: 'INSTANCE', name: 'Ref', w: 20, h: 20, x: 0, y: 0, componentId: 'start' })
      raw.others[0]!.patches = [{ at: [1], set: { x: 100, componentId: 'end' } }]
      await unsupported('non-geometry-change:ref')
    })

    it('should report an icon whose props change while it moves', async () => {
      addLayer({
        type: 'INSTANCE',
        name: 'Icon',
        w: 20,
        h: 20,
        x: 0,
        y: 0,
        component: 'Glyph',
        variant: 'active=false'
      })
      raw.others[0]!.patches = [{ at: [1], set: { x: 100, variant: 'active=true' } }]
      await unsupported('non-geometry-change:icon')
    })
  })

  describe('when generating the component', () => {
    it('should emit infinite keyframes on the moving layer and a static reduced-motion rest', async () => {
      const { files } = generateComponents([await specOf()], {})
      const css = files.find((file) => file.path.endsWith('Skeleton.module.css'))!.content

      expect(css).toContain(
        [
          '@keyframes v0loop_0 {',
          '  0% { left: -200px; animation-timing-function: linear; }',
          '  100% { left: 200px; }',
          '}'
        ].join('\n')
      )
      expect(css).toContain("& [data-n='0'] {\n  animation: v0loop_0 1500ms linear infinite;\n}")
      expect(css).toContain(
        '@media (prefers-reduced-motion: reduce) {\n  &, & [data-n] {\n    animation: none;\n  }\n}'
      )
      expect(css).not.toContain('v1loop')
    })
  })
})
