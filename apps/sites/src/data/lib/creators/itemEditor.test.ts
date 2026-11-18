import { describe, expect, it } from 'vitest'
import { BodyShape } from '@dcl/schemas'
import {
  createPanelStorage,
  getEditorMode,
  loadEditorLayout,
  pickDressedItems,
  resolveSelectedItem,
  saveEditorLayout
} from './itemEditor'
import { ItemType, type Item } from './items'

const make = (id: string, type: ItemType, bodyShapes: BodyShape[] = [BodyShape.MALE, BodyShape.FEMALE]): Item => ({
  id,
  name: id,
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type,
  data: { representations: [{ bodyShapes, mainFile: 'm.glb', contents: [] }] },
  contents: {},
  createdAt: 1,
  updatedAt: 1
})

const emote1 = make('e1', ItemType.EMOTE)
const emote2 = make('e2', ItemType.EMOTE)
const hat = make('hat', ItemType.WEARABLE)
const maleOnly = make('male', ItemType.WEARABLE, [BodyShape.MALE])

describe('editor url helpers', () => {
  it('honours reviewing=true for curators only', () => {
    const params = new URLSearchParams('reviewing=true')
    expect(getEditorMode(params, true)).toBe('review')
    expect(getEditorMode(params, false)).toBe('edit')
    expect(getEditorMode(new URLSearchParams(), true)).toBe('edit')
  })

  it('selects the url item, else the first wearable, else the first item', () => {
    expect(resolveSelectedItem([emote1, hat], 'hat')?.id).toBe('hat')
    expect(resolveSelectedItem([emote1, hat], 'missing')?.id).toBe('hat')
    expect(resolveSelectedItem([emote1, emote2], null)?.id).toBe('e1')
    expect(resolveSelectedItem([], 'x')).toBeNull()
  })
})

describe('pickDressedItems', () => {
  it('keeps dressed items with a representation for the body shape and at most the latest emote', () => {
    const dressed = pickDressedItems(
      [hat, maleOnly, emote1, emote2],
      ['maleOnly', 'male', 'e1', 'hat', 'e2'],
      BodyShape.FEMALE
    )
    expect(dressed.map(item => item.id)).toEqual(['hat', 'e2'])
    expect(pickDressedItems([maleOnly], ['male'], BodyShape.MALE)).toHaveLength(1)
  })
})

describe('editor layout persistence', () => {
  function memoryStorage() {
    const map = new Map<string, string>()
    return { getItem: (k: string) => map.get(k) ?? null, setItem: (k: string, v: string) => void map.set(k, v) }
  }

  it('remembers the sidebar mode and every panel group layout under one key', () => {
    const store = memoryStorage()
    saveEditorLayout({ sidebarCollapsed: true }, store)
    const panels = createPanelStorage(store)
    panels.setItem('item-editor', '{"a":1}')
    expect(loadEditorLayout(store)).toEqual({ sidebarCollapsed: true, panels: { 'item-editor': '{"a":1}' } })
    expect(panels.getItem('item-editor')).toBe('{"a":1}')
    expect(panels.getItem('other')).toBeNull()
  })

  it('survives garbage and a missing storage', () => {
    const store = memoryStorage()
    store.setItem('item-editor.layout', '{not json')
    expect(loadEditorLayout(store)).toEqual({})
    expect(loadEditorLayout(null)).toEqual({})
    expect(() => saveEditorLayout({ sidebarCollapsed: false }, null)).not.toThrow()
  })
})
