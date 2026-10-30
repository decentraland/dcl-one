// from: shop@94584a5c:app/src/lib/suggestionNavigation.ts
export type SuggestionRowKind = "facet" | "item" | "collection" | "creator" | "see-all" | "recent" | "popular"

export type SuggestionActivation = "click" | "keyboard"

export type SuggestionRow = {
  // The DOM id of the row, referenced by the input's aria-activedescendant.
  id: string
  kind: SuggestionRowKind
  // What choosing the row does, whether by click or by Enter.
  activate: (via: SuggestionActivation) => void
}

export const NO_ACTIVE_ROW = -1

/** The listbox's DOM id, which the combobox input points at with aria-controls. */
export const SUGGESTIONS_LISTBOX_ID = "search-suggestions"

/**
 * The next active index for a navigation key, or null when the key is not one. Arrows wrap; Home and
 * End jump. With no rows nothing is ever active.
 */
export function nextActiveIndex(current: number, count: number, key: string): number | null {
  if (key !== "ArrowDown" && key !== "ArrowUp" && key !== "Home" && key !== "End") return null
  if (count <= 0) return NO_ACTIVE_ROW
  switch (key) {
    case "ArrowDown":
      return current < 0 || current >= count - 1 ? 0 : current + 1
    case "ArrowUp":
      return current <= 0 ? count - 1 : current - 1
    case "Home":
      return 0
    default:
      return count - 1
  }
}

export type SearchKey = {
  key: string
  altKey?: boolean
  ctrlKey?: boolean
  metaKey?: boolean
  shiftKey?: boolean
  // The key is part of an IME composition (or reported as "Process"): it belongs to the text, not to us.
  isComposing?: boolean
}

export type SearchKeyState = { open: boolean; activeIndex: number; count: number }

export type SearchKeyAction =
  { type: "move"; index: number } | { type: "activate" } | { type: "submit" } | { type: "close" } | { type: "clear" }

/**
 * What a key pressed in the search box should do, or null to leave it to the field.
 *
 * Text editing wins whenever it could be meant: a modifier, an IME composition, or Home/End while no row
 * is active (they move the caret until the arrows have entered the list). Escape puts an open panel
 * away and, pressed again, clears the box through the same path the clear button takes. Enter activates
 * the active row, or submits the search when none is.
 */
export function searchKeyAction(key: SearchKey, state: SearchKeyState): SearchKeyAction | null {
  if (key.isComposing || key.key === "Process") return null
  if (key.altKey || key.ctrlKey || key.metaKey) return null
  if (key.key === "Escape") return state.open ? { type: "close" } : { type: "clear" }
  if (key.shiftKey) return null
  if (key.key === "Enter") return state.open && state.activeIndex >= 0 ? { type: "activate" } : { type: "submit" }
  if (!state.open) return null
  if ((key.key === "Home" || key.key === "End") && state.activeIndex < 0) return null
  const index = nextActiveIndex(state.activeIndex, state.count, key.key)
  return index === null ? null : { type: "move", index }
}

/**
 * A DOM id for a row that no other row can share: the key's bytes in hex, so "a b" and "a-b" differ and
 * any script survives, prefixed by the kind.
 */
export function suggestionRowId(kind: SuggestionRowKind, key: string): string {
  const hex = Array.from(new TextEncoder().encode(key), b => b.toString(16).padStart(2, "0")).join("")
  return `search-row-${kind}-${hex}`
}
