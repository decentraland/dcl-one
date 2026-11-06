import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { beforeEach, describe, expect, it } from 'vitest'
import type { RawNode } from '../ir/raw-components.js'

type Marker = {
  name: string
  type: string
  componentProperties: { ready?: { type: string; value: string } }
  getMainComponentAsync: () => Promise<{ parent: { id: string } }>
}

const script = readFileSync(
  fileURLToPath(new URL('../../capture/extract-components.figma.js', import.meta.url)),
  'utf8'
)
const start = script.indexOf('async function syncStateOf(page)')
const end = script.indexOf('\nasync function componentsOf(pageId)', start)
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor as new (
  source: string
) => () => Promise<(page: { children: Marker[] }) => Promise<string | null>>
const loadSyncState = new AsyncFunction(`${script.slice(start, end)}\nreturn syncStateOf`)

describe('when serializing effective Figma paint and border properties', () => {
  let serialize: (
    node: Record<string, unknown>,
    depth: number,
    maxDepth: number
  ) => Promise<RawNode>
  let node: Record<string, unknown>

  beforeEach(() => {
    serialize = new Function(
      'figma',
      `${script.slice(0, script.indexOf('function parseVariantName'))}\nreturn serialize`
    )({ mixed: Symbol('mixed') }) as typeof serialize
    node = {
      type: 'FRAME',
      name: 'Selected tab',
      width: 85,
      height: 64,
      strokes: [{ type: 'SOLID', color: { r: 1, g: 0, b: 0 } }],
      strokeWeight: 4,
      strokeAlign: 'INSIDE',
      strokeTopWeight: 0,
      strokeRightWeight: 0,
      strokeBottomWeight: 4,
      strokeLeftWeight: 0
    }
  })

  it('should preserve zero widths and the bottom stroke independently', async () => {
    expect(await serialize(node, 0, 0)).toMatchObject({
      strokeWeights: [0, 0, 4, 0],
      strokeAlign: 'INSIDE'
    })
  })

  describe('and a gradient contains small transform coefficients', () => {
    beforeEach(() => {
      node.fills = [
        {
          type: 'GRADIENT_LINEAR',
          gradientStops: [],
          gradientTransform: [
            [0.923456, -0.134567, 0.264321],
            [0.000123, 0.003456, 0.5]
          ]
        }
      ]
    })

    it('should retain the transform precision needed to reconstruct the gradient', async () => {
      expect(await serialize(node, 0, 0)).toMatchObject({
        fills: [
          {
            transform: [
              [0.923456, -0.134567, 0.264321],
              [0.000123, 0.003456, 0.5]
            ]
          }
        ]
      })
    })
  })
})

describe('when a page has a SyncStatus control', () => {
  let marker: Marker

  beforeEach(() => {
    marker = {
      name: 'Sync status',
      type: 'INSTANCE',
      componentProperties: { ready: { type: 'VARIANT', value: 'true' } },
      getMainComponentAsync: async () => ({ parent: { id: '15973:15' } })
    }
  })

  it('should admit a ready page', async () => {
    const syncStateOf = await loadSyncState()
    expect(await syncStateOf({ children: [marker] })).toBeNull()
  })

  it('should exclude a disabled page', async () => {
    marker.componentProperties.ready!.value = 'false'
    const syncStateOf = await loadSyncState()
    expect(await syncStateOf({ children: [marker] })).toBe('not-ready')
  })

  it('should exclude a control from another component', async () => {
    marker.getMainComponentAsync = async () => ({ parent: { id: 'another-component' } })
    const syncStateOf = await loadSyncState()
    expect(await syncStateOf({ children: [marker] })).toBe('invalid-sync-status')
  })

  it('should exclude an unknown ready value', async () => {
    marker.componentProperties.ready!.value = 'pending'
    const syncStateOf = await loadSyncState()
    expect(await syncStateOf({ children: [marker] })).toBe('invalid-sync-state')
  })

  it('should exclude a control with no ready property', async () => {
    marker.componentProperties = {}
    const syncStateOf = await loadSyncState()
    expect(await syncStateOf({ children: [marker] })).toBe('invalid-sync-state')
  })
})

describe('when a page has no unambiguous SyncStatus control', () => {
  it('should exclude a missing control', async () => {
    const syncStateOf = await loadSyncState()
    expect(await syncStateOf({ children: [] })).toBe('missing-sync-status')
  })

  it('should exclude duplicate controls', async () => {
    const syncStateOf = await loadSyncState()
    const marker = { name: 'Sync status', type: 'INSTANCE' } as Marker
    expect(await syncStateOf({ children: [marker, marker] })).toBe('missing-sync-status')
  })

  it('should exclude a non-instance control', async () => {
    const syncStateOf = await loadSyncState()
    const marker = { name: 'Sync status', type: 'FRAME' } as Marker
    expect(await syncStateOf({ children: [marker] })).toBe('missing-sync-status')
  })
})
