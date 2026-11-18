// Pure helpers behind the item editor page: URL → mode / selection, dressing rules and the persisted layout.
import { type BodyShape } from '@dcl/schemas'
import { ItemType, type Item } from './items'

export type EditorMode = 'edit' | 'review'

/** `?reviewing=true` only counts for curators; everyone else gets the editor. */
export function getEditorMode(searchParams: URLSearchParams, isCurator: boolean): EditorMode {
  return isCurator && searchParams.get('reviewing') === 'true' ? 'review' : 'edit'
}

/** The `?item=` item when it belongs to the collection, else the first wearable, else the first item. */
export function resolveSelectedItem(items: Item[], itemId: string | null): Item | null {
  if (items.length === 0) return null
  const requested = itemId ? items.find(item => item.id === itemId) : undefined
  if (requested) return requested
  return items.find(item => item.type === ItemType.WEARABLE) ?? items[0]
}

export function hasRepresentationFor(item: Item, bodyShape: BodyShape): boolean {
  return item.data.representations.some(representation => representation.bodyShapes.includes(bodyShape))
}

/** Dressed items that exist for the body shape; at most one emote, the most recently dressed. */
export function pickDressedItems(items: Item[], dressedItemIds: string[], bodyShape: BodyShape): Item[] {
  const byId = new Map(items.map(item => [item.id, item]))
  const dressed = dressedItemIds.flatMap(id => {
    const item = byId.get(id)
    return item && hasRepresentationFor(item, bodyShape) ? [item] : []
  })
  const lastEmote = [...dressed].reverse().find(item => item.type === ItemType.EMOTE)
  return dressed.filter(item => item.type !== ItemType.EMOTE || item === lastEmote)
}

export function groupItemsByType(items: Item[]): { wearables: Item[]; emotes: Item[] } {
  return {
    wearables: items.filter(item => item.type === ItemType.WEARABLE),
    emotes: items.filter(item => item.type === ItemType.EMOTE)
  }
}

export const EDITOR_LAYOUT_KEY = 'item-editor.layout'

export type EditorLayout = {
  sidebarCollapsed?: boolean
  /** Serialized panel layouts by group id, as react-resizable-panels hands them to its storage. */
  panels?: Record<string, string>
}

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>

function storage(): StorageLike | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage
  } catch {
    return null
  }
}

export function loadEditorLayout(store: StorageLike | null = storage()): EditorLayout {
  try {
    const raw = store?.getItem(EDITOR_LAYOUT_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as EditorLayout
    const panels = parsed.panels && typeof parsed.panels === 'object' ? parsed.panels : undefined
    return {
      sidebarCollapsed: typeof parsed.sidebarCollapsed === 'boolean' ? parsed.sidebarCollapsed : undefined,
      panels
    }
  } catch {
    return {}
  }
}

export function saveEditorLayout(patch: EditorLayout, store: StorageLike | null = storage()): void {
  try {
    const current = loadEditorLayout(store)
    const next: EditorLayout = {
      ...current,
      ...patch,
      panels: patch.panels ? { ...current.panels, ...patch.panels } : current.panels
    }
    store?.setItem(EDITOR_LAYOUT_KEY, JSON.stringify(next))
  } catch {
    // Private mode / quota: the layout just isn't remembered.
  }
}

/** A react-resizable-panels storage that keeps every group's layout inside the editor layout entry. */
export function createPanelStorage(store: StorageLike | null = storage()) {
  return {
    getItem: (name: string) => loadEditorLayout(store).panels?.[name] ?? null,
    setItem: (name: string, value: string) => saveEditorLayout({ panels: { [name]: value } }, store)
  }
}
