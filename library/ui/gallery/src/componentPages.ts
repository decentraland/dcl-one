// The Components section's pages: every shared component page, as the sidebar groups them, and the Showcase that
// draws them all on one canvas.
import { entries } from './examples'
import type { GalleryEntry } from './registry'

/** The page with every component page's examples in one canvas, as the HUD's Showcase shows its sections. */
export const COMPONENT_OVERVIEW = 'Showcase'

export type ComponentGroup = { name: string; entries: readonly GalleryEntry[] }

/** The pages by group, the groups in the order their first page appears: the sidebar's order. */
export function groupEntries(pages: readonly GalleryEntry[]): ComponentGroup[] {
  const groups = new Map<string, GalleryEntry[]>()
  for (const page of pages) {
    const group = groups.get(page.group)
    if (group) group.push(page)
    else groups.set(page.group, [page])
  }
  return [...groups].map(([name, grouped]) => ({ name, entries: grouped }))
}

export const componentGroups: readonly ComponentGroup[] = groupEntries(entries)

/**
 * The `component` pages. The Showcase comes last, so a link without a page, or with an unknown one, still opens the
 * first component page.
 */
export const componentPages: readonly string[] = [
  ...entries.map((entry) => entry.name),
  COMPONENT_OVERVIEW
]
