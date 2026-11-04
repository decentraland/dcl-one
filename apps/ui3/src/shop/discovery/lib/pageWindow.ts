// from: shop@3a322cf9:app/src/lib/pageWindow.ts

export const PAGE_SLOTS = 7;

/**
 * Which page numbers a pager draws, always in the same number of slots.
 *
 * The slot count never changes once a list has more pages than slots: the first and last page, the current
 * one with its neighbours, and an ellipsis for each skipped run. Near either end the window slides instead
 * of shrinking, so stepping back from the last page never adds a number and never moves the arrows.
 */
export function pageWindow(page: number, pages: number): (number | "gap")[] {
  if (pages <= PAGE_SLOTS) return Array.from({ length: pages }, (_, i) => i);
  const last = pages - 1;
  const current = Math.min(Math.max(0, page), last);
  if (current <= 3) return [0, 1, 2, 3, 4, "gap", last];
  if (current >= last - 3) return [0, "gap", last - 4, last - 3, last - 2, last - 1, last];
  return [0, "gap", current - 1, current, current + 1, "gap", last];
}
