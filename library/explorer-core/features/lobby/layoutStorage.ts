// The lobby layout, persisted as one JSON value: widgets and corners by name, so a renamed or resized widget is
// dropped on load instead of landing on a wrong one. The shape is versioned
// ({"Version":1,"Entries":[{"Corner","Widget","X","Y"}]}) so a later layout can migrate it.
import { getPref, PREF, setPref } from '../../lib/prefs'
import { LOBBY_CORNERS, LOBBY_WIDGETS, type LobbyCorner, type LobbyLayout, type LobbyWidgetId } from './layout'

const VERSION = 1

interface StoredEntry {
  Corner?: unknown
  Widget?: unknown
  X?: unknown
  Y?: unknown
}

export function serializeLayout(layout: LobbyLayout): string {
  return JSON.stringify({
    Version: VERSION,
    Entries: layout.all.map((p) => ({ Corner: p.corner, Widget: p.widget, X: p.cell.x, Y: p.cell.y }))
  })
}

/** Replaces the layout's placements with the stored ones. Unknown, out-of-grid or overlapping entries are
 *  skipped; a missing, malformed or other-version value gives the default layout. */
export function deserializeLayout(layout: LobbyLayout, stored: string | null): void {
  layout.clear()
  if (stored == null) return layout.resetToDefault()
  let value: unknown
  try {
    value = JSON.parse(stored)
  } catch {
    return layout.resetToDefault()
  }
  const dto = value as { Version?: unknown; Entries?: unknown } | null
  if (dto == null || typeof dto !== 'object' || dto.Version !== VERSION || !Array.isArray(dto.Entries)) return layout.resetToDefault()
  for (const entry of dto.Entries as StoredEntry[]) {
    if (entry == null || typeof entry !== 'object') continue
    const widget = entry.Widget
    const corner = entry.Corner
    if (typeof widget !== 'string' || !(LOBBY_WIDGETS as readonly string[]).includes(widget)) continue
    if (typeof corner !== 'string' || !(LOBBY_CORNERS as readonly string[]).includes(corner)) continue
    const x = typeof entry.X === 'number' && Number.isInteger(entry.X) ? entry.X : 0
    const y = typeof entry.Y === 'number' && Number.isInteger(entry.Y) ? entry.Y : 0
    layout.tryPlace(widget as LobbyWidgetId, corner as LobbyCorner, { x, y })
  }
}

export function loadLayout(layout: LobbyLayout): void {
  deserializeLayout(layout, getPref(PREF.lobbyLayout))
}

export function saveLayout(layout: LobbyLayout): void {
  setPref(PREF.lobbyLayout, serializeLayout(layout))
}
