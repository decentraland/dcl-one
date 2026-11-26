// Which sidebar groups a viewer collapsed, kept in this browser. Storage is a convenience: when it is missing,
// blocked or holds something else, every group starts expanded and the sidebar works the same.

export const COLLAPSED_GROUPS_KEY = 'dcl-one-ui-gallery:collapsed-groups'

/** The stored collapsed groups that still exist; none when storage is unavailable or holds anything else. */
export function readCollapsedGroups(known: readonly string[]): ReadonlySet<string> {
  try {
    const stored: unknown = JSON.parse(window.localStorage.getItem(COLLAPSED_GROUPS_KEY) ?? '[]')
    if (!Array.isArray(stored)) return new Set()
    return new Set(known.filter((id) => stored.includes(id)))
  } catch {
    return new Set()
  }
}

/** Keeps the collapsed groups for the next visit; a browser that refuses the write loses only that. */
export function writeCollapsedGroups(collapsed: ReadonlySet<string>): void {
  try {
    window.localStorage.setItem(COLLAPSED_GROUPS_KEY, JSON.stringify([...collapsed]))
  } catch {
    // Private windows and blocked site data refuse the write; the state lasts for this visit.
  }
}

export function toggleGroup(collapsed: ReadonlySet<string>, id: string): ReadonlySet<string> {
  const next = new Set(collapsed)
  if (!next.delete(id)) next.add(id)
  return next
}

export function expandGroup(collapsed: ReadonlySet<string>, id: string): ReadonlySet<string> {
  if (!collapsed.has(id)) return collapsed
  const next = new Set(collapsed)
  next.delete(id)
  return next
}

export function allCollapsed(collapsed: ReadonlySet<string>, known: readonly string[]): boolean {
  return known.length > 0 && known.every((id) => collapsed.has(id))
}

/** Collapses every group, or expands them all when every one is already collapsed. */
export function toggleAllGroups(
  collapsed: ReadonlySet<string>,
  known: readonly string[]
): ReadonlySet<string> {
  return allCollapsed(collapsed, known) ? new Set() : new Set(known)
}
